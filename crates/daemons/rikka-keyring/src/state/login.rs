use rikka_keyring_vault::{Keyring as Vault, Opener};
use rikka_secrets_protocol::names::{LOGIN_COLLECTION, SESSION_COLLECTION};
use zbus::zvariant::OwnedObjectPath;
use zeroize::Zeroizing;

use crate::fault::{Answer, Fault};
use crate::state::{Change, LOGIN_LABEL, State};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoginOutcome {
    Created,
    Unlocked,
    Changed,
    Refused,
}

impl LoginOutcome {
    pub(crate) fn name(self) -> &'static str {
        match self {
            LoginOutcome::Created => "created",
            LoginOutcome::Unlocked => "unlocked",
            LoginOutcome::Changed => "changed",
            LoginOutcome::Refused => "refused",
        }
    }
}

impl State {
    pub(crate) fn login(&mut self, password: &[u8]) -> Answer<LoginOutcome> {
        let before = self.locked_names();
        let outcome = match self.collections.get_mut(LOGIN_COLLECTION) {
            None => {
                self.create_login(password)?;
                LoginOutcome::Created
            }
            Some(login) if login.is_locked() => {
                if login.unlock(Opener::Password(password))? {
                    self.refused = None;
                    LoginOutcome::Unlocked
                } else {
                    self.refused = Some(Zeroizing::new(password.to_vec()));
                    self.changes.push(Change::Keyring);
                    LoginOutcome::Refused
                }
            }
            Some(login) => {
                if login.opens_with(password)? {
                    LoginOutcome::Unlocked
                } else {
                    login.set_password(password)?;
                    tracing::info!("login keyring now follows the changed login password");
                    LoginOutcome::Changed
                }
            }
        };
        self.settle(&before);
        Ok(outcome)
    }

    pub(crate) fn answer(&mut self, prompt: &OwnedObjectPath, password: &[u8]) -> Answer<bool> {
        if !self.prompts.contains_key(prompt) {
            return Err(Fault::no_object(prompt.as_str()));
        }
        let before = self.locked_names();
        let opened = match self.collections.get_mut(LOGIN_COLLECTION) {
            None => {
                self.create_login(password)?;
                true
            }
            Some(login) if login.is_locked() => {
                let opened = login.unlock(Opener::Password(password))?;
                if opened && let Some(current) = self.refused.take() {
                    if current.as_slice() != password {
                        login.set_password(&current)?;
                        tracing::info!("login keyring now follows the changed login password");
                    }
                    self.changes.push(Change::Keyring);
                }
                opened
            }
            Some(_) => true,
        };
        self.settle(&before);
        Ok(opened)
    }

    pub(crate) fn reset(&mut self) -> Answer<()> {
        let Some(password) = self.refused.take() else {
            return Err(Fault::denied(
                "the login keyring opens with the login password",
            ));
        };
        let before = self.locked_names();
        let names: Vec<String> = self
            .collections
            .keys()
            .filter(|name| *name != SESSION_COLLECTION)
            .cloned()
            .collect();
        for name in names {
            let items = self.item_ids(&name);
            if let Some(vault) = self.collections.remove(&name) {
                vault.delete()?;
            }
            if name != LOGIN_COLLECTION {
                self.changes.push(Change::CollectionDeleted { name, items });
            }
        }
        let aliases = self.aliases.len();
        self.aliases.retain(|_, name| name == SESSION_COLLECTION);
        if aliases != self.aliases.len() {
            self.write_aliases();
        }
        self.create_login(&password)?;
        tracing::warn!("login keyring was reset to the login password");
        self.settle(&before);
        self.changes.push(Change::Keyring);
        Ok(())
    }

    pub(crate) fn set_auto_unlock(&mut self, wanted: bool) {
        self.wants_binding = Some(wanted);
        self.apply_binding();
    }

    fn create_login(&mut self, password: &[u8]) -> Answer<()> {
        let path = self.keyring_path(LOGIN_COLLECTION);
        let login = Vault::create(path, LOGIN_LABEL, Opener::Password(password))?;
        self.collections.insert(LOGIN_COLLECTION.to_owned(), login);
        self.refused = None;
        tracing::info!("login keyring created");
        Ok(())
    }
}
