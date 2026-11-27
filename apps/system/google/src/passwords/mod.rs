mod editor;
pub(crate) mod view;

use std::sync::Arc;
use std::time::{Duration, Instant};

use google_passwords::{Credential, Draft, Recipe, Refresh, Status};
use rikka_app::prelude::*;
use rikka_ui::clipboard;

use crate::session::Session;

const POLL: &str = "google-passwords-poll";
const WATCH: &str = "google-passwords-watch";
const WATCH_PERIOD: Duration = Duration::from_secs(10 * 60);
const FRESH_FOR: Duration = Duration::from_secs(60);

pub(crate) struct Editing {
    pub(crate) replacing: Option<String>,
    pub(crate) site: String,
    pub(crate) address: TextDocument,
    pub(crate) username: TextDocument,
    pub(crate) password: TextDocument,
    pub(crate) note: TextDocument,
    pub(crate) revealed: bool,
    pub(crate) refusal: Option<String>,
    pub(crate) key: u64,
}

#[derive(State)]
pub(crate) struct PasswordState {
    #[untracked]
    session: Option<Arc<Session>>,
    credentials: Tracked<Arc<Vec<Credential>>>,
    status: Tracked<Option<Status>>,
    syncing: Tracked<bool>,
    failure: Tracked<Option<String>>,
    signed_out: Tracked<bool>,
    #[untracked]
    search: TextDocument,
    query: Tracked<String>,
    selected: Tracked<Option<String>>,
    revealed: Tracked<bool>,
    #[untracked]
    editing: Option<Editing>,
    edited: Revision,
    #[untracked]
    edits: u64,
    deleting: Tracked<Option<String>>,
    #[untracked]
    passphrase: TextDocument,
    passphrase_refused: Tracked<bool>,
    #[untracked]
    synced_at: Option<Instant>,
    #[untracked]
    resync: Option<Option<Refresh>>,
    #[untracked]
    watched: bool,
}

impl Default for PasswordState {
    fn default() -> PasswordState {
        PasswordState {
            session: None,
            credentials: Tracked::new(Arc::new(Vec::new())),
            status: Tracked::new(None),
            syncing: Tracked::new(false),
            failure: Tracked::new(None),
            signed_out: Tracked::new(false),
            search: TextDocument::new(""),
            query: Tracked::new(String::new()),
            selected: Tracked::new(None),
            revealed: Tracked::new(false),
            editing: None,
            edited: Revision::new(),
            edits: 0,
            deleting: Tracked::new(None),
            passphrase: TextDocument::new(""),
            passphrase_refused: Tracked::new(false),
            synced_at: None,
            resync: None,
            watched: false,
        }
    }
}

impl PasswordState {
    pub(crate) fn status(&self) -> Option<Status> {
        *self.status
    }

    pub(crate) fn is_syncing(&self) -> bool {
        *self.syncing
    }

    pub(crate) fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    pub(crate) fn is_signed_out(&self) -> bool {
        *self.signed_out
    }

    pub(crate) fn search(&self) -> &TextDocument {
        &self.search
    }

    pub(crate) fn passphrase(&self) -> &TextDocument {
        &self.passphrase
    }

    pub(crate) fn passphrase_refused(&self) -> bool {
        *self.passphrase_refused
    }

    pub(crate) fn is_revealed(&self) -> bool {
        *self.revealed
    }

    pub(crate) fn editing(&self) -> Option<&Editing> {
        self.edited.read();
        self.editing.as_ref()
    }

    pub(crate) fn deleting(&self) -> Option<&Credential> {
        let id = self.deleting.as_deref()?;
        self.credentials
            .iter()
            .find(|credential| credential.id == id)
    }

    pub(crate) fn shown(&self) -> Vec<&Credential> {
        let needle = self.query.to_lowercase();
        self.credentials
            .iter()
            .filter(|credential| credential.matches(&needle))
            .collect()
    }

    pub(crate) fn total(&self) -> usize {
        self.credentials.len()
    }

    pub(crate) fn selected(&self) -> Option<&Credential> {
        let id = self.selected.as_deref()?;
        self.credentials
            .iter()
            .find(|credential| credential.id == id)
    }

    pub(crate) fn attach(&mut self, session: Arc<Session>) {
        self.detach();
        let poll = session.vault.poll_interval();
        self.session = Some(session);
        self.reload();
        self.sync(None);
        every_keyed(POLL, poll, |passwords: &mut PasswordState| {
            passwords.sync(Some(Refresh::Periodic));
        });
        every_keyed(
            WATCH,
            WATCH_PERIOD,
            |passwords: &mut PasswordState| match passwords.watched {
                true => passwords.sync(Some(Refresh::Trigger)),
                false => skip_view(),
            },
        );
    }

    pub(crate) fn detach(&mut self) {
        cancel(POLL);
        cancel(WATCH);
        *self = PasswordState::default();
    }

    pub(crate) fn watch(&mut self, watched: bool) {
        self.watched = watched;
        let fresh = self
            .synced_at
            .is_some_and(|synced_at| synced_at.elapsed() < FRESH_FOR);
        if watched && !fresh {
            self.sync(Some(Refresh::Trigger));
        }
    }

    pub(crate) fn sync(&mut self, refresh: Option<Refresh>) {
        let Some(session) = &self.session else {
            return;
        };
        if *self.syncing {
            let pending = self.resync.flatten();
            self.resync = Some(match pending {
                Some(Refresh::Trigger) => pending,
                _ => refresh.or(pending),
            });
            return;
        }
        self.syncing.set(true);
        let vault = session.vault.clone();
        let key = session.key.clone();
        spawn(
            async move { (key, vault.sync(refresh).await) },
            |passwords: &mut PasswordState,
             (key, synced): (String, google_passwords::Result<Status>)| {
                passwords.synced(&key, synced);
            },
        );
    }

    pub(crate) fn search_changed(&mut self) {
        self.query.set(self.search.text().trim().to_owned());
    }

    pub(crate) fn select(&mut self, id: Option<String>) {
        if *self.selected != id {
            self.selected.set(id);
            self.revealed.set(false);
        }
    }

    pub(crate) fn toggle_revealed(&mut self) {
        let revealed = !*self.revealed;
        self.revealed.set(revealed);
    }

    pub(crate) fn copy_username(&mut self) {
        if let Some(credential) = self.selected() {
            clipboard::set_text(&credential.username);
        }
        skip_view();
    }

    pub(crate) fn copy_password(&mut self) {
        if let Some(credential) = self.selected()
            && let Err(error) =
                clipboard::publish(rikka_ui::Offer::text(&credential.password).secret())
        {
            tracing::warn!("failed to copy the password: {error}");
        }
        skip_view();
    }

    pub(crate) fn open_site(&mut self) {
        if let Some(credential) = self.selected()
            && !credential.site.address.is_empty()
            && let Err(error) = rikka_open::open(&credential.site.address)
        {
            self.failure.set(Some(error.to_string()));
        }
    }

    pub(crate) fn add(&mut self) {
        self.edit(None, &Draft::default(), String::new());
    }

    pub(crate) fn edit_selected(&mut self) {
        let Some(credential) = self.selected() else {
            return;
        };
        let (id, draft, site) = (
            credential.id.clone(),
            Draft::from_credential(credential),
            credential.site.name.clone(),
        );
        self.edit(Some(id), &draft, site);
    }

    pub(crate) fn editing_revealed(&mut self) {
        if let Some(editing) = &mut self.editing {
            editing.revealed = !editing.revealed;
            self.edited.changed();
        }
    }

    pub(crate) fn generate(&mut self) {
        if let Some(editing) = &mut self.editing {
            editing.password.set_text(Recipe::default().generate());
            editing.revealed = true;
            self.edited.changed();
        }
    }

    pub(crate) fn cancel_editing(&mut self) {
        self.editing = None;
        self.edited.changed();
    }

    pub(crate) fn save(&mut self) {
        let (Some(session), Some(editing)) = (&self.session, &mut self.editing) else {
            return;
        };
        let draft = Draft {
            address: editing.address.text(),
            username: editing.username.text(),
            password: editing.password.text(),
            note: editing.note.text(),
        };
        match session.vault.save(&draft, editing.replacing.as_deref()) {
            Ok(id) => {
                self.editing = None;
                self.edited.changed();
                self.reload();
                self.select(Some(id));
                self.sync(None);
            }
            Err(error) => {
                editing.refusal = Some(view::refusal(&error));
                self.edited.changed();
            }
        }
    }

    pub(crate) fn ask_delete(&mut self) {
        let id = (*self.selected).clone();
        self.deleting.set(id);
    }

    pub(crate) fn answer_delete(&mut self, confirmed: bool) {
        let Some(id) = self.deleting.take() else {
            return;
        };
        let Some(session) = self.session.as_ref().filter(|_| confirmed) else {
            return;
        };
        match session.vault.delete(&id) {
            Ok(()) => {
                self.select(None);
                self.reload();
                self.sync(None);
            }
            Err(error) => self.failure.set(Some(error.to_string())),
        }
    }

    pub(crate) fn unlock(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        let vault = session.vault.clone();
        let key = session.key.clone();
        let passphrase = self.passphrase.text();
        spawn_blocking(
            move || (key, vault.offer_passphrase(&passphrase)),
            |passwords: &mut PasswordState,
             (key, accepted): (String, google_passwords::Result<bool>)| {
                passwords.unlocked(&key, accepted);
            },
        );
    }

    pub(crate) fn offer_vault_keys(&mut self, keys: &[Vec<u8>]) {
        let Some(session) = &self.session else {
            return;
        };
        let vault = session.vault.clone();
        let key = session.key.clone();
        let keys = keys.to_vec();
        spawn_blocking(
            move || (key, vault.offer_vault_keys(&keys)),
            |passwords: &mut PasswordState,
             (key, accepted): (String, google_passwords::Result<bool>)| {
                passwords.vault_keys_offered(&key, accepted);
            },
        );
    }

    pub(crate) fn dismiss_failure(&mut self) {
        self.failure.set(None);
    }

    fn unlocked(&mut self, key: &str, accepted: google_passwords::Result<bool>) {
        if !self.is_current(key) {
            return skip_view();
        }
        match accepted {
            Ok(true) => {
                self.passphrase.clear();
                self.passphrase_refused.set(false);
                self.reload();
                self.sync(None);
            }
            Ok(false) => self.passphrase_refused.set(true),
            Err(error) => self.failure.set(Some(error.to_string())),
        }
    }

    fn vault_keys_offered(&mut self, key: &str, accepted: google_passwords::Result<bool>) {
        if !self.is_current(key) {
            return skip_view();
        }
        match accepted {
            Ok(true) => {
                self.failure.set(None);
                self.reload();
                self.sync(None);
            }
            Ok(false) => self
                .failure
                .set(Some(tr!("google.passwords.keys-refused").to_string())),
            Err(error) => self.failure.set(Some(error.to_string())),
        }
    }

    fn is_current(&self, key: &str) -> bool {
        self.session
            .as_ref()
            .is_some_and(|session| session.key == key)
    }

    fn edit(&mut self, replacing: Option<String>, draft: &Draft, site: String) {
        self.edits += 1;
        self.editing = Some(Editing {
            replacing,
            site,
            address: TextDocument::new(&draft.address),
            username: TextDocument::new(&draft.username),
            password: TextDocument::new(&draft.password),
            note: TextDocument::new(&draft.note),
            revealed: false,
            refusal: None,
            key: self.edits,
        });
        self.edited.changed();
    }

    fn synced(&mut self, key: &str, synced: google_passwords::Result<Status>) {
        if !self.is_current(key) {
            return skip_view();
        }
        self.syncing.set(false);
        self.synced_at = Some(Instant::now());
        match synced {
            Ok(_) => self.failure.set(None),
            Err(error) => {
                if error.is_signed_out() {
                    self.signed_out.set(true);
                }
                self.failure.set(Some(error.to_string()));
            }
        }
        self.reload();
        if let Some(refresh) = self.resync.take() {
            self.sync(refresh);
        }
    }

    fn reload(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        let status = session.vault.status();
        let changed = self
            .status
            .is_none_or(|known| known.revision != status.revision);
        if *self.status != Some(status) {
            self.status.set(Some(status));
        }
        if !changed {
            return;
        }
        self.credentials.set(Arc::new(session.vault.credentials()));
        if self.selected.is_some() && self.selected().is_none() {
            self.selected.set(None);
        }
    }
}
