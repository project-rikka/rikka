use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::ErrorKind;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hkdf::Hkdf;
use rikka_fs::Replace;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::format::{
    ARGON2_VERSION, ARGON2ID, ContentsRecord, FORMAT, FileRecord, ItemRecord, KdfRecord, SlotRecord,
};
use crate::item::{Attributes, Item, NewItem, Secret, Stored};
use crate::kdf::{PasswordKdf, SALT_BYTES};
use crate::locked::{KEY_BYTES, KeyPage, KeyRole};
use crate::seal::{NONCE_BYTES, Sealed, framed};

const ID_BYTES: usize = 16;
const FIRST_ITEM: u64 = 1;
const FOLDER_MODE: u32 = 0o700;
const LOCK_MODE: u32 = 0o600;
const LOCK_FILE: &str = ".lock";
const MEMORY: &str = "(memory)";

const DOMAIN: &[u8] = b"rikka-keyring";
const VERSION_TAG: &[u8] = b"1";
const CONTENTS: &[u8] = b"contents";
const ITEM: &[u8] = b"item";
const SLOT: &[u8] = b"slot";
const PASSWORD_SLOT: &[u8] = b"password";
const WRAPPED_SLOT: &[u8] = b"wrapped";
const BOUND_SLOT: &[u8] = b"bound";

const CONTENTS_INFO: &[u8] = b"rikka-keyring v1 contents";
const ITEMS_INFO: &[u8] = b"rikka-keyring v1 items";
const WRAP_INFO: &[u8] = b"rikka-keyring v1 wrap";

#[derive(Clone, Copy, Debug)]
pub enum Opener<'a> {
    Password(&'a [u8]),
    Keyring(&'a Keyring),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding<'a> {
    pub name: &'a str,
    pub blob: &'a [u8],
}

pub struct Keyring {
    place: Place,
    id: [u8; ID_BYTES],
    label: String,
    created: u64,
    modified: u64,
    slots: Vec<Slot>,
    sealed: Sealed,
    seen: Option<FileIdentity>,
    open: Option<Opened>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Place {
    File(PathBuf),
    Memory,
}

struct Snapshot {
    label: String,
    modified: u64,
    slots: Vec<Slot>,
    sealed: Sealed,
    contents: Option<(u64, Vec<StoredItem>)>,
}

struct Opened {
    keys: KeyPage,
    next: u64,
    items: Vec<StoredItem>,
}

#[derive(Clone)]
struct StoredItem {
    item: Item,
    secret: Sealed,
}

#[derive(Clone)]
enum Slot {
    Password {
        kdf: PasswordKdf,
        wrapped: Sealed,
    },
    Wrapped {
        by: [u8; ID_BYTES],
        wrapped: Sealed,
    },
    Bound {
        name: String,
        blob: Vec<u8>,
        wrapped: Sealed,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
    len: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
}

struct Loaded {
    record: Header,
    identity: FileIdentity,
}

struct Header {
    id: [u8; ID_BYTES],
    label: String,
    created: u64,
    modified: u64,
    slots: Vec<Slot>,
    sealed: Sealed,
}

impl Keyring {
    pub fn create(path: impl Into<PathBuf>, label: &str, opener: Opener<'_>) -> Result<Keyring> {
        let path = path.into();
        let _guard = FolderLock::take(&path)?;
        if path.exists() {
            return Err(Error::Exists { path });
        }
        let mut keyring = Keyring::fresh(Place::File(path), label)?;
        let slot = new_slot(
            opener,
            &keyring.id,
            keyring.opened()?.keys.key(KeyRole::Master),
        )?;
        keyring.slots.push(slot);
        keyring.write()?;
        Ok(keyring)
    }

    pub fn ephemeral(label: &str) -> Result<Keyring> {
        Keyring::fresh(Place::Memory, label)
    }

    fn fresh(place: Place, label: &str) -> Result<Keyring> {
        let mut id = [0u8; ID_BYTES];
        getrandom::fill(&mut id).map_err(Error::Random)?;
        let mut keys = KeyPage::new()?;
        getrandom::fill(keys.key_mut(KeyRole::Master)).map_err(Error::Random)?;
        derive_keys(&mut keys, &id)?;
        let now = now();
        Ok(Keyring {
            place,
            id,
            label: label.to_owned(),
            created: now,
            modified: now,
            slots: Vec::new(),
            sealed: Sealed {
                nonce: [0; NONCE_BYTES],
                ciphertext: Vec::new(),
            },
            seen: None,
            open: Some(Opened {
                keys,
                next: FIRST_ITEM,
                items: Vec::new(),
            }),
        })
    }

    pub fn open(path: impl Into<PathBuf>) -> Result<Option<Keyring>> {
        let path = path.into();
        let Some(loaded) = load(&path)? else {
            return Ok(None);
        };
        let Header {
            id,
            label,
            created,
            modified,
            slots,
            sealed,
        } = loaded.record;
        Ok(Some(Keyring {
            place: Place::File(path),
            id,
            label,
            created,
            modified,
            slots,
            sealed,
            seen: Some(loaded.identity),
            open: None,
        }))
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match &self.place {
            Place::File(path) => Some(path),
            Place::Memory => None,
        }
    }

    #[must_use]
    pub fn is_ephemeral(&self) -> bool {
        matches!(self.place, Place::Memory)
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn created(&self) -> u64 {
        self.created
    }

    #[must_use]
    pub fn modified(&self) -> u64 {
        self.modified
    }

    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.open.is_none()
    }

    #[must_use]
    pub fn binding(&self) -> Option<Binding<'_>> {
        self.slots.iter().find_map(|slot| match slot {
            Slot::Bound { name, blob, .. } => Some(Binding { name, blob }),
            _ => None,
        })
    }

    pub fn refresh(&mut self) -> Result<bool> {
        let Place::File(path) = &self.place else {
            return Ok(false);
        };
        let current = identity_at(path)?;
        if current.is_some() && current == self.seen {
            return Ok(false);
        }
        let Some(loaded) = load(path)? else {
            return Err(Error::Replaced { path: path.clone() });
        };
        if loaded.record.id != self.id {
            return Err(Error::Replaced { path: path.clone() });
        }
        let Header {
            label,
            created,
            modified,
            slots,
            sealed,
            ..
        } = loaded.record;
        self.label = label;
        self.created = created;
        self.modified = modified;
        self.slots = slots;
        self.sealed = sealed;
        self.seen = Some(loaded.identity);
        if let Some(opened) = self.open.take() {
            self.open = Some(self.reopen(opened.keys)?);
        }
        Ok(true)
    }

    pub fn unlock(&mut self, opener: Opener<'_>) -> Result<bool> {
        if self.open.is_some() {
            return Ok(true);
        }
        self.refresh()?;
        let master = match opener {
            Opener::Password(password) => self.open_password_slot(password)?,
            Opener::Keyring(wrapper) => {
                let wrap = wrapper.opened()?.keys.key(KeyRole::Wrap);
                let aad = slot_aad(WRAPPED_SLOT, &self.id);
                self.slots.iter().find_map(|slot| match slot {
                    Slot::Wrapped { by, wrapped } if *by == wrapper.id => wrapped.open(wrap, &aad),
                    _ => None,
                })
            }
        };
        match master {
            Some(master) => self.adopt(&master).map(|()| true),
            None => Ok(false),
        }
    }

    pub fn unlock_bound(&mut self, slot_key: &[u8]) -> Result<bool> {
        if self.open.is_some() {
            return Ok(true);
        }
        self.refresh()?;
        let Ok(slot_key) = <&[u8; KEY_BYTES]>::try_from(slot_key) else {
            return Ok(false);
        };
        let aad = slot_aad(BOUND_SLOT, &self.id);
        let master = self.slots.iter().find_map(|slot| match slot {
            Slot::Bound { wrapped, .. } => wrapped.open(slot_key, &aad),
            _ => None,
        });
        match master {
            Some(master) => self.adopt(&master).map(|()| true),
            None => Ok(false),
        }
    }

    pub fn opens_with(&self, password: &[u8]) -> Result<bool> {
        Ok(self.open_password_slot(password)?.is_some())
    }

    pub fn lock(&mut self) {
        if !self.is_ephemeral() {
            self.open = None;
        }
    }

    pub fn set_password(&mut self, password: &[u8]) -> Result<()> {
        let slot = new_slot(
            Opener::Password(password),
            &self.id,
            self.opened()?.keys.key(KeyRole::Master),
        )?;
        self.commit(|keyring| {
            keyring
                .slots
                .retain(|slot| !matches!(slot, Slot::Password { .. }));
            keyring.slots.push(slot);
            Ok(())
        })
    }

    pub fn bind<E: std::fmt::Display>(
        &mut self,
        name: &str,
        seal: impl FnOnce(&[u8]) -> Result<Vec<u8>, E>,
    ) -> Result<()> {
        let mut slot_key = Zeroizing::new([0u8; KEY_BYTES]);
        getrandom::fill(slot_key.as_mut_slice()).map_err(Error::Random)?;
        let wrapped = Sealed::seal(
            &slot_key,
            &slot_aad(BOUND_SLOT, &self.id),
            self.opened()?.keys.key(KeyRole::Master),
        )?;
        let blob = seal(slot_key.as_slice()).map_err(|error| Error::Binding(error.to_string()))?;
        let slot = Slot::Bound {
            name: name.to_owned(),
            blob,
            wrapped,
        };
        self.commit(|keyring| {
            keyring
                .slots
                .retain(|slot| !matches!(slot, Slot::Bound { .. }));
            keyring.slots.push(slot);
            Ok(())
        })
    }

    pub fn unbind(&mut self) -> Result<bool> {
        if self.binding().is_none() {
            return Ok(false);
        }
        self.commit(|keyring| {
            keyring
                .slots
                .retain(|slot| !matches!(slot, Slot::Bound { .. }));
            Ok(true)
        })
    }

    pub fn set_label(&mut self, label: &str) -> Result<()> {
        self.commit(|keyring| {
            keyring.label = label.to_owned();
            Ok(())
        })
    }

    pub fn delete(self) -> Result<()> {
        let Place::File(path) = &self.place else {
            return Ok(());
        };
        let _guard = FolderLock::take(path)?;
        match fs::remove_file(path) {
            Err(error) if error.kind() != ErrorKind::NotFound => Err(Error::Io {
                path: path.clone(),
                source: error,
            }),
            _ => Ok(()),
        }
    }

    pub fn items(&self) -> Result<Vec<&Item>> {
        Ok(self
            .opened()?
            .items
            .iter()
            .map(|stored| &stored.item)
            .collect())
    }

    pub fn search(&self, query: &Attributes) -> Result<Vec<&Item>> {
        Ok(self
            .opened()?
            .items
            .iter()
            .map(|stored| &stored.item)
            .filter(|item| item.matches(query))
            .collect())
    }

    pub fn item(&self, id: u64) -> Result<&Item> {
        self.opened()?
            .items
            .iter()
            .map(|stored| &stored.item)
            .find(|item| item.id == id)
            .ok_or(Error::NoSuchItem(id))
    }

    pub fn secret(&self, id: u64) -> Result<Secret> {
        let opened = self.opened()?;
        let stored = opened
            .items
            .iter()
            .find(|stored| stored.item.id == id)
            .ok_or(Error::NoSuchItem(id))?;
        let value = stored
            .secret
            .open(opened.keys.key(KeyRole::Items), &item_aad(&self.id, id))
            .ok_or_else(|| Error::Tampered { path: self.shown() })?;
        Ok(Secret {
            value,
            content_type: stored.item.content_type.clone(),
        })
    }

    pub fn store(&mut self, new: NewItem<'_>, replace: bool) -> Result<Stored> {
        let collection = self.id;
        self.commit(|keyring| {
            let opened = keyring.opened_mut()?;
            let now = now();
            let existing = replace
                .then(|| {
                    opened
                        .items
                        .iter()
                        .position(|stored| stored.item.attributes == new.attributes)
                })
                .flatten();
            let id = match existing {
                Some(at) => opened.items[at].item.id,
                None => {
                    let id = opened.next;
                    opened.next += 1;
                    id
                }
            };
            let secret = Sealed::seal(
                opened.keys.key(KeyRole::Items),
                &item_aad(&collection, id),
                new.secret,
            )?;
            match existing {
                Some(at) => {
                    let stored = &mut opened.items[at];
                    stored.item.label = new.label.to_owned();
                    stored.item.content_type = new.content_type.to_owned();
                    stored.item.modified = now;
                    stored.secret = secret;
                }
                None => opened.items.push(StoredItem {
                    item: Item {
                        id,
                        label: new.label.to_owned(),
                        attributes: new.attributes,
                        content_type: new.content_type.to_owned(),
                        created: now,
                        modified: now,
                    },
                    secret,
                }),
            }
            Ok(Stored {
                id,
                replaced: existing.is_some(),
            })
        })
    }

    pub fn set_secret(&mut self, id: u64, secret: &[u8], content_type: &str) -> Result<()> {
        let collection = self.id;
        self.edit_item(id, |stored, keys| {
            stored.secret =
                Sealed::seal(keys.key(KeyRole::Items), &item_aad(&collection, id), secret)?;
            stored.item.content_type = content_type.to_owned();
            Ok(())
        })
    }

    pub fn set_item_label(&mut self, id: u64, label: &str) -> Result<()> {
        self.edit_item(id, |stored, _| {
            stored.item.label = label.to_owned();
            Ok(())
        })
    }

    pub fn set_item_attributes(&mut self, id: u64, attributes: Attributes) -> Result<()> {
        self.edit_item(id, |stored, _| {
            stored.item.attributes = attributes;
            Ok(())
        })
    }

    pub fn delete_item(&mut self, id: u64) -> Result<()> {
        self.commit(|keyring| {
            let opened = keyring.opened_mut()?;
            let at = opened
                .items
                .iter()
                .position(|stored| stored.item.id == id)
                .ok_or(Error::NoSuchItem(id))?;
            opened.items.remove(at);
            Ok(())
        })
    }

    fn edit_item(
        &mut self,
        id: u64,
        edit: impl FnOnce(&mut StoredItem, &KeyPage) -> Result<()>,
    ) -> Result<()> {
        self.commit(|keyring| {
            let opened = keyring.opened_mut()?;
            let stored = opened
                .items
                .iter_mut()
                .find(|stored| stored.item.id == id)
                .ok_or(Error::NoSuchItem(id))?;
            edit(stored, &opened.keys)?;
            stored.item.modified = now();
            Ok(())
        })
    }

    fn commit<T>(&mut self, change: impl FnOnce(&mut Keyring) -> Result<T>) -> Result<T> {
        let _guard = match &self.place {
            Place::File(path) => Some(FolderLock::take(path)?),
            Place::Memory => None,
        };
        self.refresh()?;
        let before = self.snapshot();
        let committed = change(self).and_then(|changed| {
            if self.open.is_some() {
                self.modified = now();
            }
            self.write().map(|()| changed)
        });
        if committed.is_err() {
            self.restore(before);
        }
        committed
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            label: self.label.clone(),
            modified: self.modified,
            slots: self.slots.clone(),
            sealed: self.sealed.clone(),
            contents: self
                .open
                .as_ref()
                .map(|opened| (opened.next, opened.items.clone())),
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.label = snapshot.label;
        self.modified = snapshot.modified;
        self.slots = snapshot.slots;
        self.sealed = snapshot.sealed;
        if let (Some(opened), Some((next, items))) = (self.open.as_mut(), snapshot.contents) {
            opened.next = next;
            opened.items = items;
        }
    }

    fn shown(&self) -> PathBuf {
        match &self.place {
            Place::File(path) => path.clone(),
            Place::Memory => PathBuf::from(MEMORY),
        }
    }

    fn opened(&self) -> Result<&Opened> {
        self.open.as_ref().ok_or(Error::Locked)
    }

    fn opened_mut(&mut self) -> Result<&mut Opened> {
        self.open.as_mut().ok_or(Error::Locked)
    }

    fn open_password_slot(&self, password: &[u8]) -> Result<Option<Zeroizing<Vec<u8>>>> {
        let aad = slot_aad(PASSWORD_SLOT, &self.id);
        for slot in &self.slots {
            let Slot::Password { kdf, wrapped } = slot else {
                continue;
            };
            if !kdf.is_plausible() {
                tracing::warn!(path = %self.shown().display(), "skipping password slot with implausible cost");
                continue;
            }
            let key = kdf.derive(password)?;
            if let Some(master) = wrapped.open(&key, &aad) {
                return Ok(Some(master));
            }
        }
        Ok(None)
    }

    fn adopt(&mut self, master: &[u8]) -> Result<()> {
        let master: &[u8; KEY_BYTES] = master
            .try_into()
            .map_err(|_| Error::Tampered { path: self.shown() })?;
        let mut keys = KeyPage::new()?;
        keys.key_mut(KeyRole::Master).copy_from_slice(master);
        derive_keys(&mut keys, &self.id)?;
        self.open = Some(self.reopen(keys)?);
        Ok(())
    }

    fn reopen(&self, keys: KeyPage) -> Result<Opened> {
        let tampered = || Error::Tampered { path: self.shown() };
        let plain = self
            .sealed
            .open(
                keys.key(KeyRole::Contents),
                &contents_aad(&self.id, self.created),
            )
            .ok_or_else(tampered)?;
        let record: ContentsRecord = serde_json::from_slice(&plain).map_err(|_| tampered())?;
        let items = record
            .items
            .into_iter()
            .map(|record| stored_item(record).ok_or_else(tampered))
            .collect::<Result<Vec<_>>>()?;
        Ok(Opened {
            keys,
            next: record.next,
            items,
        })
    }

    fn write(&mut self) -> Result<()> {
        let Place::File(path) = &self.place else {
            return Ok(());
        };
        let path = path.clone();
        if let Some(opened) = &self.open {
            let record = ContentsRecord {
                next: opened.next,
                items: opened.items.iter().map(item_record).collect(),
            };
            let plain =
                Zeroizing::new(
                    serde_json::to_vec(&record).map_err(|error| Error::Malformed {
                        path: path.clone(),
                        reason: error.to_string(),
                    })?,
                );
            self.sealed = Sealed::seal(
                opened.keys.key(KeyRole::Contents),
                &contents_aad(&self.id, self.created),
                &plain,
            )?;
        }
        let record = FileRecord {
            id: rikka_types::hex::encode(self.id),
            label: self.label.clone(),
            created: self.created,
            modified: self.modified,
            slots: self.slots.iter().map(slot_record).collect(),
            nonce: self.sealed.nonce.to_vec(),
            sealed: self.sealed.ciphertext.clone(),
        };
        let bytes = FORMAT.encode(&record).map_err(|source| Error::File {
            path: path.clone(),
            source,
        })?;
        Replace::new(&path)
            .private()
            .write(bytes)
            .map_err(|source| Error::Io {
                path: path.clone(),
                source,
            })?;
        self.seen = identity_at(&path)?;
        Ok(())
    }
}

impl std::fmt::Debug for Keyring {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Keyring")
            .field("place", &self.place)
            .field("label", &self.label)
            .field("locked", &self.is_locked())
            .finish_non_exhaustive()
    }
}

struct FolderLock {
    _held: File,
}

impl FolderLock {
    fn take(path: &Path) -> Result<FolderLock> {
        let folder = path.parent().ok_or_else(|| Error::Malformed {
            path: path.to_path_buf(),
            reason: "the keyring path has no folder".to_owned(),
        })?;
        let failed = |source| Error::Io {
            path: folder.to_path_buf(),
            source,
        };
        DirBuilder::new()
            .recursive(true)
            .mode(FOLDER_MODE)
            .create(folder)
            .map_err(failed)?;
        fs::set_permissions(folder, fs::Permissions::from_mode(FOLDER_MODE)).map_err(failed)?;
        let lock = folder.join(LOCK_FILE);
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(LOCK_MODE)
            .open(&lock)
            .map_err(failed)?;
        file.lock().map_err(failed)?;
        Ok(FolderLock { _held: file })
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn derive_keys(keys: &mut KeyPage, id: &[u8; ID_BYTES]) -> Result<()> {
    let master = Zeroizing::new(*keys.key(KeyRole::Master));
    let hkdf = Hkdf::<Sha256>::new(Some(id), master.as_slice());
    for (role, info) in [
        (KeyRole::Contents, CONTENTS_INFO),
        (KeyRole::Items, ITEMS_INFO),
        (KeyRole::Wrap, WRAP_INFO),
    ] {
        hkdf.expand(info, keys.key_mut(role))
            .map_err(|_| Error::Cipher)?;
    }
    Ok(())
}

fn new_slot(opener: Opener<'_>, id: &[u8; ID_BYTES], master: &[u8; KEY_BYTES]) -> Result<Slot> {
    match opener {
        Opener::Password(password) => {
            let kdf = PasswordKdf::fresh()?;
            let key = kdf.derive(password)?;
            let wrapped = Sealed::seal(&key, &slot_aad(PASSWORD_SLOT, id), master)?;
            Ok(Slot::Password { kdf, wrapped })
        }
        Opener::Keyring(wrapper) => {
            let wrap = wrapper.opened()?.keys.key(KeyRole::Wrap);
            let wrapped = Sealed::seal(wrap, &slot_aad(WRAPPED_SLOT, id), master)?;
            Ok(Slot::Wrapped {
                by: wrapper.id,
                wrapped,
            })
        }
    }
}

fn contents_aad(id: &[u8; ID_BYTES], created: u64) -> Vec<u8> {
    framed(&[DOMAIN, VERSION_TAG, CONTENTS, id, &created.to_be_bytes()])
}

fn item_aad(collection: &[u8; ID_BYTES], item: u64) -> Vec<u8> {
    framed(&[DOMAIN, VERSION_TAG, ITEM, collection, &item.to_be_bytes()])
}

fn slot_aad(kind: &[u8], id: &[u8; ID_BYTES]) -> Vec<u8> {
    framed(&[DOMAIN, VERSION_TAG, SLOT, kind, id])
}

fn identity_at(path: &Path) -> Result<Option<FileIdentity>> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(Some(identity(&metadata))),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(source) => Err(Error::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn identity(metadata: &fs::Metadata) -> FileIdentity {
    FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        len: metadata.len(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
    }
}

fn load(path: &Path) -> Result<Option<Loaded>> {
    let failed = |source| Error::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(failed(error)),
    };
    let identity = identity(&file.metadata().map_err(failed)?);
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut file, &mut bytes).map_err(failed)?;
    let malformed = |reason: &str| Error::Malformed {
        path: path.to_path_buf(),
        reason: reason.to_owned(),
    };
    let record: FileRecord = FORMAT.decode(&bytes).map_err(|source| Error::File {
        path: path.to_path_buf(),
        source,
    })?;
    let id = rikka_types::hex::decode(&record.id)
        .ok()
        .and_then(|id| <[u8; ID_BYTES]>::try_from(id).ok())
        .ok_or_else(|| malformed("the keyring id is not 16 bytes of hex"))?;
    let slots = record
        .slots
        .into_iter()
        .map(slot)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| malformed("a key slot is unreadable"))?;
    let sealed = sealed(record.nonce, record.sealed)
        .ok_or_else(|| malformed("the contents nonce is not 24 bytes"))?;
    Ok(Some(Loaded {
        record: Header {
            id,
            label: record.label,
            created: record.created,
            modified: record.modified,
            slots,
            sealed,
        },
        identity,
    }))
}

fn sealed(nonce: Vec<u8>, ciphertext: Vec<u8>) -> Option<Sealed> {
    Some(Sealed {
        nonce: nonce.try_into().ok()?,
        ciphertext,
    })
}

fn slot(record: SlotRecord) -> Option<Slot> {
    match record {
        SlotRecord::Password { kdf, nonce, sealed } => {
            if kdf.algorithm != ARGON2ID || kdf.version != ARGON2_VERSION {
                return None;
            }
            let salt: [u8; SALT_BYTES] = kdf.salt.try_into().ok()?;
            Some(Slot::Password {
                kdf: PasswordKdf {
                    memory_kib: kdf.memory_kib,
                    passes: kdf.passes,
                    lanes: kdf.lanes,
                    salt,
                },
                wrapped: self::sealed(nonce, sealed)?,
            })
        }
        SlotRecord::Wrapped { by, nonce, sealed } => Some(Slot::Wrapped {
            by: rikka_types::hex::decode(by).ok()?.try_into().ok()?,
            wrapped: self::sealed(nonce, sealed)?,
        }),
        SlotRecord::Bound {
            binding,
            blob,
            nonce,
            sealed,
        } => Some(Slot::Bound {
            name: binding,
            blob,
            wrapped: self::sealed(nonce, sealed)?,
        }),
    }
}

fn slot_record(slot: &Slot) -> SlotRecord {
    match slot {
        Slot::Password { kdf, wrapped } => SlotRecord::Password {
            kdf: KdfRecord {
                algorithm: ARGON2ID.to_owned(),
                version: ARGON2_VERSION,
                memory_kib: kdf.memory_kib,
                passes: kdf.passes,
                lanes: kdf.lanes,
                salt: kdf.salt.to_vec(),
            },
            nonce: wrapped.nonce.to_vec(),
            sealed: wrapped.ciphertext.clone(),
        },
        Slot::Wrapped { by, wrapped } => SlotRecord::Wrapped {
            by: rikka_types::hex::encode(by),
            nonce: wrapped.nonce.to_vec(),
            sealed: wrapped.ciphertext.clone(),
        },
        Slot::Bound {
            name,
            blob,
            wrapped,
        } => SlotRecord::Bound {
            binding: name.clone(),
            blob: blob.clone(),
            nonce: wrapped.nonce.to_vec(),
            sealed: wrapped.ciphertext.clone(),
        },
    }
}

fn stored_item(record: ItemRecord) -> Option<StoredItem> {
    Some(StoredItem {
        item: Item {
            id: record.id,
            label: record.label,
            attributes: record.attributes,
            content_type: record.content_type,
            created: record.created,
            modified: record.modified,
        },
        secret: sealed(record.nonce, record.sealed)?,
    })
}

fn item_record(stored: &StoredItem) -> ItemRecord {
    ItemRecord {
        id: stored.item.id,
        label: stored.item.label.clone(),
        attributes: stored.item.attributes.clone(),
        content_type: stored.item.content_type.clone(),
        created: stored.item.created,
        modified: stored.item.modified,
        nonce: stored.secret.nonce.to_vec(),
        sealed: stored.secret.ciphertext.clone(),
    }
}
