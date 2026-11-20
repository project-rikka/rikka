use rikka_keyring_vault::{Attributes, Keyring as Vault, Opener};
use rikka_secrets_protocol::names::{LOGIN_COLLECTION, SESSION_COLLECTION};
use zbus::zvariant::OwnedObjectPath;

use crate::fault::{Answer, Fault};
use crate::paths::{self, Target};
use crate::state::{Change, LOGIN_LABEL, State, item_ids};

impl State {
    pub(crate) fn create_collection(
        &mut self,
        label: &str,
        alias: &str,
    ) -> Answer<OwnedObjectPath> {
        if !alias.is_empty() {
            if !paths::is_element(alias) {
                return Err(Fault::invalid(format!("{alias:?} is not an alias name")));
            }
            if let Some(existing) = self.resolve_alias(alias) {
                return Ok(paths::collection(&existing));
            }
        }
        let login = self
            .collections
            .get(LOGIN_COLLECTION)
            .filter(|login| !login.is_locked())
            .ok_or_else(Fault::locked)?;
        let base = paths::element(label);
        let name = (1u64..)
            .map(|count| match count {
                1 => base.clone(),
                _ => format!("{base}_{count}"),
            })
            .find(|name| !self.exists(name) && name != SESSION_COLLECTION)
            .expect("an unused collection name exists");
        let vault = Vault::create(self.keyring_path(&name), label, Opener::Keyring(login))?;
        self.collections.insert(name.clone(), vault);
        self.changes.push(Change::CollectionCreated(name.clone()));
        if !alias.is_empty() {
            self.aliases.insert(alias.to_owned(), name.clone());
            self.write_aliases();
            self.changes.push(Change::AliasAdded(alias.to_owned()));
        }
        Ok(paths::collection(&name))
    }

    pub(crate) fn delete_collection(&mut self, name: &str) -> Answer<()> {
        if name == SESSION_COLLECTION {
            return Err(Fault::denied(
                "the session collection lasts for the session",
            ));
        }
        let before = self.locked_names();
        let items = self.item_ids(name);
        let vault = self
            .collections
            .remove(name)
            .ok_or_else(|| Fault::no_object(paths::collection(name).as_str()))?;
        vault.delete()?;
        let gone: Vec<String> = self
            .aliases
            .iter()
            .filter(|(_, target)| *target == name)
            .map(|(alias, _)| alias.clone())
            .collect();
        for alias in &gone {
            self.aliases.remove(alias);
            self.changes.push(Change::AliasRemoved(alias.clone()));
        }
        if !gone.is_empty() {
            self.write_aliases();
        }
        match name == LOGIN_COLLECTION {
            true => {
                for id in items {
                    self.changes
                        .push(Change::ItemDeleted(LOGIN_COLLECTION.to_owned(), id));
                }
                self.changes
                    .push(Change::CollectionChanged(LOGIN_COLLECTION.to_owned()));
                tracing::warn!("login keyring deleted");
            }
            false => self.changes.push(Change::CollectionDeleted {
                name: name.to_owned(),
                items,
            }),
        }
        self.settle(&before);
        Ok(())
    }

    pub(crate) fn lock(&mut self, objects: Vec<OwnedObjectPath>) -> Vec<OwnedObjectPath> {
        let mut locked = Vec::new();
        for object in objects {
            let Some(name) = self.collection_of(&object) else {
                continue;
            };
            if self.lock_collection(&name) {
                locked.push(object);
            }
        }
        locked
    }

    pub(crate) fn lock_all(&mut self) {
        for name in self.collection_names() {
            self.lock_collection(&name);
        }
    }

    fn lock_collection(&mut self, name: &str) -> bool {
        let items = self.item_ids(name);
        let Some(vault) = self.collections.get_mut(name) else {
            return true;
        };
        if vault.is_ephemeral() {
            return false;
        }
        if !vault.is_locked() {
            vault.lock();
            self.changes.push(Change::Closed {
                collection: name.to_owned(),
                items,
            });
            self.changes.push(Change::Keyring);
        }
        true
    }

    pub(crate) fn set_alias(&mut self, alias: &str, target: &OwnedObjectPath) -> Answer<()> {
        if !paths::is_element(alias) {
            return Err(Fault::invalid(format!("{alias:?} is not an alias name")));
        }
        if target.as_str() == paths::none().as_str() {
            if self.aliases.remove(alias).is_some() {
                self.write_aliases();
                self.changes.push(Change::AliasRemoved(alias.to_owned()));
            }
            return Ok(());
        }
        let Some(Target::Collection(name)) = paths::target(target) else {
            return Err(Fault::no_object(target.as_str()));
        };
        if !self.exists(&name) {
            return Err(Fault::no_object(target.as_str()));
        }
        let known = self.alias_names().iter().any(|known| known == alias);
        self.aliases.insert(alias.to_owned(), name);
        self.write_aliases();
        if !known {
            self.changes.push(Change::AliasAdded(alias.to_owned()));
        }
        Ok(())
    }

    pub(crate) fn search(&self, query: &Attributes) -> Vec<OwnedObjectPath> {
        self.collection_names()
            .iter()
            .flat_map(|name| self.search_collection(name, query))
            .collect()
    }

    pub(crate) fn search_collection(&self, name: &str, query: &Attributes) -> Vec<OwnedObjectPath> {
        self.collections
            .get(name)
            .and_then(|vault| vault.search(query).ok())
            .map(|items| {
                items
                    .iter()
                    .map(|item| paths::item(name, item.id()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn label(&self, name: &str) -> String {
        match self.collections.get(name) {
            Some(vault) => vault.label().to_owned(),
            None => LOGIN_LABEL.to_owned(),
        }
    }

    pub(crate) fn set_label(&mut self, name: &str, label: &str) -> Answer<()> {
        let vault = self.collections.get_mut(name).ok_or_else(Fault::locked)?;
        vault.set_label(label)?;
        self.changes
            .push(Change::CollectionChanged(name.to_owned()));
        Ok(())
    }

    pub(crate) fn is_locked(&self, name: &str) -> bool {
        self.collections.get(name).is_none_or(Vault::is_locked)
    }

    pub(crate) fn created(&self, name: &str) -> u64 {
        self.collections.get(name).map_or(0, Vault::created)
    }

    pub(crate) fn modified(&self, name: &str) -> u64 {
        self.collections.get(name).map_or(0, Vault::modified)
    }

    pub(crate) fn item_paths(&self, name: &str) -> Vec<OwnedObjectPath> {
        self.item_ids(name)
            .into_iter()
            .map(|id| paths::item(name, id))
            .collect()
    }

    pub(crate) fn resolve(&self, target: &Target) -> Option<String> {
        let name = match target {
            Target::Collection(name) | Target::Item(name, _) => name.clone(),
            Target::Alias(alias) => self.resolve_alias(alias)?,
        };
        self.exists(&name).then_some(name)
    }

    pub(crate) fn refresh(&mut self) {
        let mut vanished = Vec::new();
        for (name, vault) in &mut self.collections {
            let before = item_ids(vault);
            match vault.refresh() {
                Ok(false) => {}
                Ok(true) => {
                    let after = item_ids(vault);
                    for id in before.iter().filter(|id| !after.contains(id)) {
                        self.changes.push(Change::ItemDeleted(name.clone(), *id));
                    }
                    for id in after.iter().filter(|id| !before.contains(id)) {
                        self.changes.push(Change::ItemCreated(name.clone(), *id));
                    }
                    self.changes.push(Change::CollectionChanged(name.clone()));
                }
                Err(error @ rikka_keyring_vault::Error::Replaced { .. }) => {
                    tracing::warn!(%error, collection = %name, "collection changed under the keyring");
                    vanished.push((name.clone(), before));
                }
                Err(error) => {
                    tracing::warn!(%error, collection = %name, "failed to refresh collection");
                }
            }
        }
        for (name, items) in vanished {
            self.collections.remove(&name);
            match name == LOGIN_COLLECTION {
                true => {
                    for id in items {
                        self.changes.push(Change::ItemDeleted(name.clone(), id));
                    }
                    self.changes.push(Change::CollectionChanged(name));
                    self.changes.push(Change::Keyring);
                }
                false => self.changes.push(Change::CollectionDeleted { name, items }),
            }
        }
    }
}
