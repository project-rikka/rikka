use rikka_proton_pgp::{Pgp, PrivateKey};

use crate::account_keys::AccountKeys;
use crate::address::Address;
use crate::error::{Error, Result};
use crate::identifier::KeyId;
use crate::key_flags::AddressKeyFlags;
use crate::key_list::SignedKeyListItem;
use crate::signed_key_list::SignedKeyList;

const CRYPTO_REFRESH_VERSION: u8 = 6;

#[derive(Debug, Clone)]
pub struct ActiveKey {
    key: PrivateKey,
    flags: AddressKeyFlags,
    primary: bool,
}

impl ActiveKey {
    #[must_use]
    pub fn new(key: PrivateKey, flags: AddressKeyFlags, primary: bool) -> ActiveKey {
        ActiveKey {
            key,
            flags,
            primary,
        }
    }

    #[must_use]
    pub fn key(&self) -> &PrivateKey {
        &self.key
    }

    #[must_use]
    pub fn flags(&self) -> AddressKeyFlags {
        self.flags
    }

    #[must_use]
    pub fn is_primary(&self) -> bool {
        self.primary
    }
}

pub(crate) struct ActiveKeys {
    entries: Vec<KeyListEntry>,
}

struct KeyListEntry {
    id: KeyId,
    key: PrivateKey,
    item: SignedKeyListItem,
}

impl KeyListEntry {
    fn is_v6(&self) -> bool {
        self.key.version() == CRYPTO_REFRESH_VERSION
    }
}

impl ActiveKeys {
    pub(crate) fn for_address(address: &Address, keys: &AccountKeys) -> ActiveKeys {
        let published = address.signed_key_list().and_then(SignedKeyList::key_list);

        let entries = address
            .keys()
            .iter()
            .filter(|key| key.is_active())
            .filter_map(|key| {
                keys.address_key(key.id())
                    .map(|unlocked_key| (key, unlocked_key))
            })
            .filter(|(_, unlocked_key)| !unlocked_key.is_forwarding_key())
            .map(|(key, unlocked_key)| {
                let public = unlocked_key.public_key();
                let item = published
                    .as_ref()
                    .and_then(|list| list.find(&public))
                    .cloned()
                    .unwrap_or_else(|| {
                        SignedKeyListItem::new(&public, key.is_primary(), key.flags().into())
                    });

                KeyListEntry {
                    id: key.id().clone(),
                    key: unlocked_key.clone(),
                    item,
                }
            })
            .collect();

        ActiveKeys { entries }.normalized()
    }

    pub(crate) fn from_active(keys: &[ActiveKey]) -> ActiveKeys {
        let entries = keys
            .iter()
            .filter(|active| !active.key.is_forwarding_key())
            .map(|active| KeyListEntry {
                id: KeyId::default(),
                key: active.key.clone(),
                item: SignedKeyListItem::new(
                    &active.key.public_key(),
                    active.primary,
                    active.flags,
                ),
            })
            .collect();

        ActiveKeys { entries }.normalized()
    }

    pub(crate) fn has_version(&self, version: u8) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.key.version() == version)
    }

    pub(crate) fn is_primary(&self, id: &KeyId) -> bool {
        self.entries
            .iter()
            .any(|entry| &entry.id == id && entry.item.is_primary())
    }

    pub(crate) fn primary(&self) -> Option<&PrivateKey> {
        self.entries
            .iter()
            .find(|entry| !entry.is_v6() && entry.item.is_primary())
            .map(|entry| &entry.key)
    }

    pub(crate) fn with_key(
        mut self,
        id: KeyId,
        key: PrivateKey,
        flags: AddressKeyFlags,
        primary: bool,
    ) -> ActiveKeys {
        let item = SignedKeyListItem::new(&key.public_key(), primary, flags);
        let entry = KeyListEntry { id, key, item };

        match primary {
            true => {
                let v6 = entry.is_v6();
                for active_key in self
                    .entries
                    .iter_mut()
                    .filter(|active_key| active_key.is_v6() == v6)
                {
                    active_key.item = active_key.item.clone().with_primary(false);
                }
                self.entries.insert(0, entry);
            }
            false => self.entries.push(entry),
        }
        self.normalized()
    }

    pub(crate) fn with_primary(mut self, id: &KeyId) -> Result<ActiveKeys> {
        let index = self.position(id)?;
        let v6 = self.entries[index].is_v6();

        for (position, entry) in self.entries.iter_mut().enumerate() {
            if entry.is_v6() == v6 {
                entry.item = entry.item.clone().with_primary(position == index);
            }
        }
        Ok(self.normalized())
    }

    pub(crate) fn with_flags(mut self, id: &KeyId, flags: AddressKeyFlags) -> Result<ActiveKeys> {
        let index = self.position(id)?;
        let entry = &mut self.entries[index];
        entry.item = entry.item.clone().with_flags(flags);

        Ok(self)
    }

    pub(crate) fn map_flags(
        mut self,
        change: impl Fn(AddressKeyFlags) -> AddressKeyFlags,
    ) -> ActiveKeys {
        for entry in &mut self.entries {
            entry.item = entry.item.clone().with_flags(change(entry.item.flags()));
        }
        self
    }

    pub(crate) fn without_v6_primary(mut self, id: &KeyId) -> Result<ActiveKeys> {
        let index = self.position(id)?;
        let entry = &mut self.entries[index];
        if !entry.is_v6() || !entry.item.is_primary() {
            return Err(Error::NoSuchKey(id.clone()));
        }
        entry.item = entry.item.clone().with_primary(false);

        Ok(self.normalized())
    }

    pub(crate) fn without_key(mut self, id: &KeyId) -> ActiveKeys {
        self.entries.retain(|entry| &entry.id != id);
        self.normalized()
    }

    pub(crate) fn sign_if_any(&self, pgp: &Pgp) -> Result<Option<SignedKeyList>> {
        if self.entries.is_empty() {
            return Ok(None);
        }
        let signer = self.primary().ok_or(Error::KeyListWithoutPrimary)?;
        let signers: Vec<PrivateKey> = std::iter::once(signer.clone())
            .chain(
                self.entries
                    .iter()
                    .find(|entry| entry.is_v6() && entry.item.is_primary())
                    .map(|entry| entry.key.clone()),
            )
            .collect();
        let items: Vec<SignedKeyListItem> = self
            .entries
            .iter()
            .map(|entry| entry.item.clone())
            .collect();

        SignedKeyList::sign_with_keys(pgp, &items, &signers).map(Some)
    }

    pub(crate) fn sign(&self, pgp: &Pgp) -> Result<SignedKeyList> {
        self.sign_if_any(pgp)?.ok_or(Error::KeyListWithoutPrimary)
    }

    fn position(&self, id: &KeyId) -> Result<usize> {
        self.entries
            .iter()
            .position(|entry| &entry.id == id)
            .ok_or_else(|| Error::NoSuchKey(id.clone()))
    }

    fn normalized(self) -> ActiveKeys {
        let (mut v4, mut v6): (Vec<KeyListEntry>, Vec<KeyListEntry>) =
            self.entries.into_iter().partition(|entry| !entry.is_v6());
        v4.sort_by_key(|entry| !entry.item.is_primary());
        v6.sort_by_key(|entry| !entry.item.is_primary());

        for (index, entry) in v4.iter_mut().enumerate() {
            entry.item = entry.item.clone().with_primary(index == 0);
        }
        for (index, entry) in v6.iter_mut().enumerate() {
            let primary = index == 0 && entry.item.is_primary();
            entry.item = entry.item.clone().with_primary(primary);
        }

        let mut entries: Vec<KeyListEntry> = v4.into_iter().chain(v6).collect();
        entries.sort_by_key(|entry| !entry.item.is_primary());

        ActiveKeys { entries }
    }
}
