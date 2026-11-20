mod collections;
mod items;
mod login;
mod prompts;
mod sessions;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use rikka_keyring_vault::{Keyring as Vault, Opener};
use rikka_secrets_protocol::names::{
    DEFAULT_ALIAS, LOGIN_COLLECTION, SESSION_ALIAS, SESSION_COLLECTION,
};
use zbus::zvariant::OwnedObjectPath;
use zeroize::Zeroizing;

use crate::daemon::Binding;
use crate::error::{Error, Result};
use crate::paths;
use crate::systemd_creds;

pub(crate) use items::Creating;
pub(crate) use prompts::Prompt;
pub(crate) use sessions::Session;

const KEYRING_EXTENSION: &str = "keyring";
const ALIASES_FILE: &str = "aliases.json";
const LOGIN_ALIAS: &str = "login";
const LOGIN_LABEL: &str = "Login";
const SESSION_LABEL: &str = "Session";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    CollectionCreated(String),
    CollectionDeleted {
        name: String,
        items: Vec<u64>,
    },
    CollectionChanged(String),
    Opened {
        collection: String,
        items: Vec<u64>,
    },
    Closed {
        collection: String,
        items: Vec<u64>,
    },
    ItemCreated(String, u64),
    ItemDeleted(String, u64),
    ItemChanged(String, u64),
    SessionOpened(OwnedObjectPath),
    SessionClosed(OwnedObjectPath),
    PromptOpened(OwnedObjectPath),
    PromptCompleted {
        path: OwnedObjectPath,
        dismissed: bool,
        result: Vec<OwnedObjectPath>,
    },
    AliasAdded(String),
    AliasRemoved(String),
    Keyring,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyringState {
    Absent,
    Locked,
    Refused,
    Unlocked,
}

impl KeyringState {
    pub(crate) fn name(self) -> &'static str {
        match self {
            KeyringState::Absent => "absent",
            KeyringState::Locked => "locked",
            KeyringState::Refused => "refused",
            KeyringState::Unlocked => "unlocked",
        }
    }
}

pub(crate) struct State {
    folder: PathBuf,
    binding: Binding,
    collections: BTreeMap<String, Vault>,
    aliases: BTreeMap<String, String>,
    sessions: HashMap<OwnedObjectPath, Session>,
    prompts: HashMap<OwnedObjectPath, Prompt>,
    serial: u64,
    refused: Option<Zeroizing<Vec<u8>>>,
    wants_binding: Option<bool>,
    changes: Vec<Change>,
}

impl State {
    pub(crate) fn load(folder: PathBuf, binding: Binding) -> Result<State> {
        let mut collections = BTreeMap::new();
        for (name, path) in keyring_files(&folder)? {
            match Vault::open(&path) {
                Ok(Some(vault)) => {
                    collections.insert(name, vault);
                }
                Ok(None) => {}
                Err(error) if name == LOGIN_COLLECTION => return Err(Error::Login(error)),
                Err(error) => {
                    tracing::error!(%error, path = %path.display(), "skipping unreadable keyring");
                }
            }
        }
        collections.insert(
            SESSION_COLLECTION.to_owned(),
            Vault::ephemeral(SESSION_LABEL).map_err(Error::Session)?,
        );
        let aliases = read_aliases(&folder)
            .into_iter()
            .filter(|(alias, name)| paths::is_element(alias) && collections.contains_key(name))
            .collect();
        let mut state = State {
            folder,
            binding,
            collections,
            aliases,
            sessions: HashMap::new(),
            prompts: HashMap::new(),
            serial: 0,
            refused: None,
            wants_binding: None,
            changes: Vec::new(),
        };
        let before = state.locked_names();
        state.open_bound();
        state.settle(&before);
        state.changes.clear();
        Ok(state)
    }

    pub(crate) fn take_changes(&mut self) -> Vec<Change> {
        let mut changes = std::mem::take(&mut self.changes);
        let mut keyring = false;
        changes.retain(|change| match change {
            Change::Keyring => !std::mem::replace(&mut keyring, true),
            _ => true,
        });
        changes
    }

    pub(crate) fn collection_names(&self) -> Vec<String> {
        let mut names = vec![LOGIN_COLLECTION.to_owned()];
        names.extend(
            self.collections
                .keys()
                .filter(|name| *name != LOGIN_COLLECTION)
                .cloned(),
        );
        names
    }

    pub(crate) fn collection_paths(&self) -> Vec<OwnedObjectPath> {
        self.collection_names()
            .iter()
            .map(|name| paths::collection(name))
            .collect()
    }

    pub(crate) fn alias_names(&self) -> Vec<String> {
        let mut names: BTreeSet<String> = [DEFAULT_ALIAS, LOGIN_ALIAS, SESSION_ALIAS]
            .into_iter()
            .map(str::to_owned)
            .collect();
        names.extend(self.aliases.keys().cloned());
        names.into_iter().collect()
    }

    pub(crate) fn resolve_alias(&self, alias: &str) -> Option<String> {
        if let Some(name) = self.aliases.get(alias) {
            return Some(name.clone());
        }
        match alias {
            DEFAULT_ALIAS | LOGIN_ALIAS => Some(LOGIN_COLLECTION.to_owned()),
            SESSION_ALIAS => Some(SESSION_COLLECTION.to_owned()),
            _ => None,
        }
    }

    pub(crate) fn keyring_state(&self) -> KeyringState {
        match self.collections.get(LOGIN_COLLECTION) {
            None => KeyringState::Absent,
            Some(_) if self.refused.is_some() => KeyringState::Refused,
            Some(login) if login.is_locked() => KeyringState::Locked,
            Some(_) => KeyringState::Unlocked,
        }
    }

    pub(crate) fn auto_unlock(&self) -> bool {
        self.collections
            .get(LOGIN_COLLECTION)
            .and_then(Vault::binding)
            .is_some()
    }

    fn exists(&self, name: &str) -> bool {
        name == LOGIN_COLLECTION || self.collections.contains_key(name)
    }

    fn locked_names(&self) -> BTreeSet<String> {
        self.collection_names()
            .into_iter()
            .filter(|name| self.collections.get(name).is_none_or(Vault::is_locked))
            .collect()
    }

    fn item_ids(&self, name: &str) -> Vec<u64> {
        self.collections.get(name).map(item_ids).unwrap_or_default()
    }

    fn next_serial(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }

    fn keyring_path(&self, name: &str) -> PathBuf {
        self.folder.join(format!("{name}.{KEYRING_EXTENSION}"))
    }

    fn open_bound(&mut self) {
        if self.binding != Binding::SystemdCredentials {
            return;
        }
        let Some(login) = self.collections.get_mut(LOGIN_COLLECTION) else {
            return;
        };
        let Some(binding) = login.binding() else {
            return;
        };
        if !login.is_locked() || binding.name != systemd_creds::BINDING_NAME {
            return;
        }
        let opened = systemd_creds::decrypt(binding.blob)
            .map_err(|error| error.to_string())
            .and_then(|slot_key| {
                login
                    .unlock_bound(&slot_key)
                    .map_err(|error| error.to_string())
            });
        match opened {
            Ok(true) => tracing::info!("login keyring unlocked through systemd-creds"),
            Ok(false) => tracing::warn!("systemd-creds binding no longer opens the login keyring"),
            Err(error) => {
                tracing::warn!(%error, "failed to unlock the login keyring through systemd-creds")
            }
        }
    }

    fn open_wrapped(&mut self) {
        let Some(login) = self.collections.remove(LOGIN_COLLECTION) else {
            return;
        };
        if !login.is_locked() {
            for (name, vault) in &mut self.collections {
                if !vault.is_locked() {
                    continue;
                }
                match vault.unlock(Opener::Keyring(&login)) {
                    Ok(true) => {}
                    Ok(false) => {
                        tracing::warn!(collection = %name, "login keyring does not open this collection")
                    }
                    Err(error) => {
                        tracing::warn!(%error, collection = %name, "failed to open collection")
                    }
                }
            }
        }
        self.collections.insert(LOGIN_COLLECTION.to_owned(), login);
    }

    fn apply_binding(&mut self) {
        if self.binding != Binding::SystemdCredentials {
            return;
        }
        let Some(wanted) = self.wants_binding else {
            return;
        };
        let Some(login) = self.collections.get_mut(LOGIN_COLLECTION) else {
            return;
        };
        let bound = login.binding().is_some();
        let outcome = match (wanted, bound) {
            (true, false) if !login.is_locked() => login
                .bind(systemd_creds::BINDING_NAME, systemd_creds::encrypt)
                .map(|()| true),
            (false, true) => login.unbind(),
            _ => Ok(false),
        };
        match outcome {
            Ok(true) => self.changes.push(Change::Keyring),
            Ok(false) => {}
            Err(error) => tracing::warn!(%error, wanted, "failed to change automatic unlocking"),
        }
    }

    fn settle(&mut self, before: &BTreeSet<String>) {
        self.open_wrapped();
        self.apply_binding();
        let after = self.locked_names();
        for name in before.difference(&after) {
            let items = self.item_ids(name);
            self.changes.push(Change::Opened {
                collection: name.clone(),
                items,
            });
        }
        for name in after.difference(before) {
            self.changes.push(Change::Closed {
                collection: name.clone(),
                items: Vec::new(),
            });
        }
        self.complete_satisfied();
        if before != &after {
            self.changes.push(Change::Keyring);
        }
    }

    fn write_aliases(&self) {
        let path = self.folder.join(ALIASES_FILE);
        let written = serde_json::to_vec_pretty(&self.aliases)
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                rikka_fs::Replace::new(&path)
                    .private()
                    .write(bytes)
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = written {
            tracing::warn!(%error, path = %path.display(), "failed to save collection aliases");
        }
    }
}

impl std::fmt::Debug for State {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("State")
            .field("folder", &self.folder)
            .field("collections", &self.collections)
            .field("sessions", &self.sessions.len())
            .field("prompts", &self.prompts.len())
            .finish_non_exhaustive()
    }
}

fn item_ids(vault: &Vault) -> Vec<u64> {
    vault
        .items()
        .map(|items| items.iter().map(|item| item.id()).collect())
        .unwrap_or_default()
}

fn keyring_files(folder: &Path) -> Result<Vec<(String, PathBuf)>> {
    let failed = |source| Error::Folder {
        path: folder.to_path_buf(),
        source,
    };
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(failed(error)),
    };
    let mut found = Vec::new();
    for entry in entries {
        let path = entry.map_err(failed)?.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some(KEYRING_EXTENSION) {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if paths::element(name) == name && name != SESSION_COLLECTION {
            found.push((name.to_owned(), path));
        }
    }
    Ok(found)
}

fn read_aliases(folder: &Path) -> BTreeMap<String, String> {
    let path = folder.join(ALIASES_FILE);
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
            tracing::warn!(%error, path = %path.display(), "ignoring unreadable collection aliases");
            BTreeMap::new()
        }),
        Err(error) if error.kind() == ErrorKind::NotFound => BTreeMap::new(),
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "failed to read collection aliases");
            BTreeMap::new()
        }
    }
}
