use std::sync::Arc;

use discord_client::Link;
use discord_core::{Account, Area, SavedAccount, Update, User};
use rikka_app::prelude::*;

use super::Chat;
use crate::account::LinkedAccount;

const RESUME: &str = "discord-resume";
const UPDATES: &str = "discord-updates";
const SIGN_OUT: &str = "discord-sign-out";

impl Chat {
    pub(crate) fn start(&mut self) {
        self.listen_for_notices();
        self.watch_krisp();
        let current_key = self.accounts.current().map(|saved| saved.key.clone());
        self.pending_account.set(current_key);
        for saved in self.accounts.all().to_vec() {
            self.resume(&saved);
        }
    }

    pub(crate) fn switch(&mut self, key: &str) {
        self.sidebar.close_menu();
        if self.current.as_deref() == Some(key) {
            return;
        }
        if self.is_linked(key) {
            self.error_message.set(None);
            self.choose(key);
            return;
        }
        let Some(saved) = self.accounts.find(key).cloned() else {
            return;
        };
        self.error_message.set(None);
        self.pending_account.set(Some(saved.key.clone()));
        self.current.set(None);
        self.resume(&saved);
    }

    pub(crate) fn retry(&mut self) {
        let Some(saved) = self.pending_account().cloned() else {
            return;
        };
        self.error_message.set(None);
        self.resume(&saved);
    }

    pub(crate) fn sign_out(&mut self, key: &str) {
        self.sidebar.close_menu();
        if let Some(linked) = self.linked(key) {
            let client = linked.client().clone();
            let name = linked.account().name.clone();
            spawn_keyed(
                (SIGN_OUT, key.to_owned()),
                async move { client.sign_out().await },
                move |result: discord_core::Result<()>| {
                    if let Err(error) = result {
                        tracing::warn!("failed to sign out {name} remotely: {error}");
                    }
                    skip_view();
                },
            );
        }
        self.forget(key);
    }

    pub(crate) fn forget(&mut self, key: &str) {
        cancel(resume_key(key));
        self.connection_error.remove(key);
        self.preferences.forget_account(key);
        if let Err(error) = self.accounts.forget(key) {
            self.error_message.set(Some(error.to_string()));
        }
        if self.pending_account.as_deref() == Some(key) {
            self.pending_account.set(None);
        }
        self.unlink(key);
        if self.current.is_none() && self.pending_account.is_none() {
            self.switch_to_next_account();
        }
    }

    pub(crate) fn resumed(&mut self, saved: &SavedAccount, resumed: Result<Option<Link>, String>) {
        if self.accounts.find(&saved.key).is_none() {
            return;
        }
        let is_pending = self.pending_account.as_deref() == Some(saved.key.as_str());
        match resumed {
            Ok(Some(link)) => {
                self.connection_error.remove(&saved.key);
                self.describe(&saved.key, &link.account);
                self.link(&saved.key, link);
                if is_pending || (self.current.is_none() && self.pending_account.is_none()) {
                    self.choose(&saved.key);
                }
            }
            Ok(None) => {
                let caption = tr!(
                    "discord.accounts.expired",
                    name = saved.account.name.as_str()
                )
                .to_string();
                self.forget(&saved.key);
                self.error_message.set(Some(caption));
            }
            Err(reason) => {
                tracing::warn!("{} unreachable: {reason}", saved.account.name);
                if is_pending {
                    self.error_message.set(Some(reason.clone()));
                }
                self.connection_error.insert(saved.key.clone(), reason);
            }
        }
    }

    pub(crate) fn connected(&mut self, key: String, link: Link) {
        self.error_message.set(None);
        self.connection_error.remove(&key);
        let saved_keys: Vec<String> = self
            .accounts
            .all()
            .iter()
            .map(|known| known.key.clone())
            .collect();
        let replaced: Vec<String> = self
            .links
            .iter()
            .map(|linked| linked.key().to_owned())
            .filter(|linked| !saved_keys.contains(linked))
            .collect();
        for replaced_key in replaced {
            cancel(updates_key(&replaced_key));
            self.unlink(&replaced_key);
        }
        self.link(&key, link);
        self.choose(&key);
    }

    pub(crate) fn apply_update(&mut self, key: &str, update: Update) {
        let update = match update {
            Update::Notify(notice) => {
                self.notice(key, notice);
                return;
            }
            update => update,
        };
        let me = match &update {
            Update::Me(me) => Some(me.clone()),
            Update::Ready(ready) => Some(ready.me.clone()),
            _ => None,
        };
        let arrived = match &update {
            Update::Space(space) => Some(space.id),
            _ => None,
        };
        let Some(linked) = self.linked_mut(key) else {
            return;
        };
        let areas = linked.apply(update);
        if let Some(me) = me {
            self.describe(key, &account_of(&me));
        }
        if self.current.as_deref() == Some(key) && areas.contains(&Area::Everything) {
            self.arrive();
        }
        if let Some(space) = arrived {
            self.space_arrived(key, &space);
        }
    }

    fn resume(&mut self, saved: &SavedAccount) {
        let discord = Arc::clone(&self.discord);
        let folder = self.accounts.folder(&saved.key);
        let saved = saved.clone();
        spawn_keyed(
            resume_key(&saved.key),
            async move {
                discord
                    .resume(&folder)
                    .await
                    .map_err(|error| error.to_string())
            },
            move |chat: &mut Chat, resumed: Result<Option<Link>, String>| {
                chat.resumed(&saved, resumed);
            },
        );
    }

    fn link(&mut self, key: &str, link: Link) {
        let (linked, updates) = LinkedAccount::new(key, link);
        match self.links.iter().position(|existing| existing.key() == key) {
            Some(at) => self.links[at] = linked,
            None => self.links.push(linked),
        }
        self.joined.changed();
        let account = key.to_owned();
        stream_keyed(
            updates_key(key),
            updates,
            move |chat: &mut Chat, update: Update| chat.apply_update(&account, update),
        );
    }

    fn unlink(&mut self, key: &str) {
        cancel(updates_key(key));
        let before = self.links.len();
        self.links.retain(|linked| linked.key() != key);
        if self.links.len() == before {
            return;
        }
        self.joined.changed();
        self.close_overlays();
        if self.calls.seat().is_some_and(|seat| seat.key == key) {
            self.leave_voice();
        }
        if self.current.as_deref() == Some(key) {
            self.current.set(None);
        }
    }

    fn switch_to_next_account(&mut self) {
        if let Some(next) = self.links.first().map(|linked| linked.key().to_owned()) {
            self.choose(&next);
            return;
        }
        let next = self.accounts.current().cloned();
        if let Some(next) = next {
            self.pending_account.set(Some(next.key.clone()));
            if !self.connection_error.contains_key(&next.key) {
                return;
            }
            self.resume(&next);
        }
    }

    pub(crate) fn choose(&mut self, key: &str) {
        if !self.is_linked(key) {
            return;
        }
        self.pending_account.set(None);
        self.current.set(Some(key.to_owned()));
        if let Err(error) = self.accounts.choose(key) {
            tracing::warn!("failed to save current account: {error}");
        }
        self.arrive();
    }

    fn describe(&mut self, key: &str, account: &Account) {
        if let Some(linked) = self.linked_mut(key) {
            linked.describe(account.clone());
        }
        let changed = self
            .accounts
            .find(key)
            .is_some_and(|known| known.account != *account);
        if changed
            && let Err(error) = self
                .accounts
                .update(key, |saved| saved.account = account.clone())
        {
            tracing::warn!("failed to save account: {error}");
        }
    }
}

fn account_of(me: &User) -> Account {
    Account {
        id: me.id,
        name: me.name().to_owned(),
        avatar: me.avatar.as_ref().map(|picture| picture.url_for_size(128)),
    }
}

fn resume_key(key: &str) -> (&'static str, String) {
    (RESUME, key.to_owned())
}

fn updates_key(key: &str) -> (&'static str, String) {
    (UPDATES, key.to_owned())
}
