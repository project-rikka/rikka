use rikka_keyring_vault::{Attributes, Item, Keyring as Vault, NewItem};
use rikka_secrets_protocol::WireSecret;
use rikka_secrets_protocol::names::BINARY_CONTENT;
use zbus::zvariant::OwnedObjectPath;

use crate::fault::{Answer, Fault, Kind};
use crate::paths::{self, Target};
use crate::state::{Change, State};

pub(crate) struct Creating {
    pub(crate) label: String,
    pub(crate) attributes: Attributes,
    pub(crate) secret: WireSecret,
    pub(crate) replace: bool,
}

impl State {
    pub(crate) fn create_item(
        &mut self,
        collection: &str,
        caller: &str,
        creating: Creating,
    ) -> Answer<OwnedObjectPath> {
        let plain = self.reveal(&creating.secret, caller)?;
        let content_type = content_type(&creating.secret);
        let vault = self.open_vault_mut(collection)?;
        let stored = vault.store(
            NewItem {
                label: &creating.label,
                attributes: creating.attributes,
                content_type,
                secret: &plain,
            },
            creating.replace,
        )?;
        self.changes.push(match stored.replaced {
            true => Change::ItemChanged(collection.to_owned(), stored.id),
            false => Change::ItemCreated(collection.to_owned(), stored.id),
        });
        Ok(paths::item(collection, stored.id))
    }

    pub(crate) fn delete_item(&mut self, collection: &str, id: u64) -> Answer<()> {
        self.open_vault_mut(collection)?.delete_item(id)?;
        self.changes
            .push(Change::ItemDeleted(collection.to_owned(), id));
        Ok(())
    }

    pub(crate) fn secret(
        &self,
        collection: &str,
        id: u64,
        session: &OwnedObjectPath,
        caller: &str,
    ) -> Answer<WireSecret> {
        let secret = self.open_vault(collection)?.secret(id)?;
        self.conceal(session, caller, &secret.value, &secret.content_type)
    }

    pub(crate) fn secrets(
        &self,
        items: &[OwnedObjectPath],
        session: &OwnedObjectPath,
        caller: &str,
    ) -> Answer<Vec<(OwnedObjectPath, WireSecret)>> {
        let mut found = Vec::new();
        for path in items {
            let Some(Target::Item(collection, id)) = paths::target(path) else {
                continue;
            };
            match self.secret(&collection, id, session, caller) {
                Ok(secret) => found.push((path.clone(), secret)),
                Err(fault) if matches!(fault.kind(), Kind::IsLocked | Kind::NoSuchObject) => {}
                Err(fault) => return Err(fault),
            }
        }
        Ok(found)
    }

    pub(crate) fn set_secret(
        &mut self,
        collection: &str,
        id: u64,
        secret: &WireSecret,
        caller: &str,
    ) -> Answer<()> {
        let plain = self.reveal(secret, caller)?;
        self.open_vault_mut(collection)?
            .set_secret(id, &plain, content_type(secret))?;
        self.changes
            .push(Change::ItemChanged(collection.to_owned(), id));
        Ok(())
    }

    pub(crate) fn item(&self, collection: &str, id: u64) -> Answer<&Item> {
        Ok(self.open_vault(collection)?.item(id)?)
    }

    pub(crate) fn set_item_label(&mut self, collection: &str, id: u64, label: &str) -> Answer<()> {
        self.open_vault_mut(collection)?.set_item_label(id, label)?;
        self.changes
            .push(Change::ItemChanged(collection.to_owned(), id));
        Ok(())
    }

    pub(crate) fn set_item_attributes(
        &mut self,
        collection: &str,
        id: u64,
        attributes: Attributes,
    ) -> Answer<()> {
        self.open_vault_mut(collection)?
            .set_item_attributes(id, attributes)?;
        self.changes
            .push(Change::ItemChanged(collection.to_owned(), id));
        Ok(())
    }

    fn open_vault(&self, collection: &str) -> Answer<&Vault> {
        self.collections
            .get(collection)
            .filter(|vault| !vault.is_locked())
            .ok_or_else(Fault::locked)
    }

    fn open_vault_mut(&mut self, collection: &str) -> Answer<&mut Vault> {
        self.collections
            .get_mut(collection)
            .filter(|vault| !vault.is_locked())
            .ok_or_else(Fault::locked)
    }
}

fn content_type(secret: &WireSecret) -> &str {
    match secret.content_type.is_empty() {
        true => BINARY_CONTENT,
        false => &secret.content_type,
    }
}
