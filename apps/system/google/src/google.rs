use std::sync::Arc;
use std::time::Duration;

use google_account::{Master, Profile, SavedAccount};
use rikka_account_book::{Accounts, PendingAccount};
use rikka_app::prelude::*;
use rikka_web::Event as PageEvent;

use crate::drive::DriveState;
use crate::mail::MailState;
use crate::passwords::PasswordState;
use crate::portal::{Portal, Purpose};
use crate::session::{Failure, Roots, Session, identified};

const RESUME: &str = "google-resume";
const PORTAL: &str = "google-portal";
const TOKEN: &str = "google-portal-token";
const TOKEN_WATCH: &str = "google-portal-token-watch";
const TOKEN_WATCH_PERIOD: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Section {
    Mail,
    Drive,
    Passwords,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Starting,
    Welcome,
    Page(Purpose),
    Connecting,
    Ready,
}

struct Pending {
    account: PendingAccount,
    master: Master,
    profile: Profile,
}

#[derive(State)]
pub(crate) struct Google {
    #[untracked]
    roots: Roots,
    accounts: Tracked<Accounts<SavedAccount>>,
    stage: Tracked<Stage>,
    section: Tracked<Section>,
    failure: Tracked<Option<String>>,
    account_menu: Tracked<bool>,
    #[untracked]
    session: Option<Arc<Session>>,
    #[untracked]
    portal: Option<Portal>,
    #[untracked]
    pending: Option<Pending>,
    #[state]
    mail: MailState,
    #[state]
    drive: DriveState,
    #[state]
    passwords: PasswordState,
}

impl Section {
    pub(crate) const ALL: [Section; 3] = [Section::Mail, Section::Drive, Section::Passwords];

    pub(crate) fn index(self) -> usize {
        Section::ALL
            .iter()
            .position(|section| *section == self)
            .unwrap_or(0)
    }

    pub(crate) fn symbol(self) -> MaterialSymbol {
        match self {
            Section::Mail => MaterialSymbol::MAIL,
            Section::Drive => MaterialSymbol::ADD_TO_DRIVE,
            Section::Passwords => MaterialSymbol::KEY,
        }
    }

    pub(crate) fn title(self) -> Label {
        match self {
            Section::Mail => tr!("google.section.mail"),
            Section::Drive => tr!("google.section.drive"),
            Section::Passwords => tr!("google.section.passwords"),
        }
    }
}

impl Google {
    pub(crate) fn new(roots: Roots, accounts: Accounts<SavedAccount>) -> Google {
        Google {
            roots,
            accounts: Tracked::new(accounts),
            stage: Tracked::new(Stage::Starting),
            section: Tracked::new(Section::ALL[0]),
            failure: Tracked::new(None),
            account_menu: Tracked::new(false),
            session: None,
            portal: None,
            pending: None,
            mail: MailState::default(),
            drive: DriveState::default(),
            passwords: PasswordState::default(),
        }
    }

    pub(crate) fn stage(&self) -> Stage {
        *self.stage
    }

    pub(crate) fn section(&self) -> Section {
        *self.section
    }

    pub(crate) fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    pub(crate) fn accounts(&self) -> &[SavedAccount] {
        self.accounts.all()
    }

    pub(crate) fn current(&self) -> Option<&SavedAccount> {
        self.accounts.current()
    }

    pub(crate) fn account_menu(&self) -> bool {
        *self.account_menu
    }

    pub(crate) fn portal(&self) -> Option<&Portal> {
        self.portal.as_ref()
    }

    pub(crate) fn needs_sign_in(&self) -> bool {
        self.mail.is_signed_out() || self.drive.is_signed_out() || self.passwords.is_signed_out()
    }

    pub(crate) fn start(&mut self) {
        match self.accounts.current().cloned() {
            Some(saved) => self.resume(saved),
            None => self.stage.set(Stage::Welcome),
        }
    }

    pub(crate) fn select(&mut self, section: Section) {
        self.section.set(section);
        self.mail.watch(section == Section::Mail);
        self.drive.watch(section == Section::Drive);
        self.passwords.watch(section == Section::Passwords);
    }

    pub(crate) fn command(&mut self, command: Command) {
        let engaged = *self.stage != Stage::Ready
            || self.mail.composing().is_some()
            || self.drive.asking().is_some()
            || self.passwords.editing().is_some();
        if engaged {
            return skip_view();
        }
        match (*self.section, command) {
            (Section::Mail, Command::Refresh) => self.mail.refresh(),
            (Section::Mail, Command::Find) => self.mail.seek(),
            (Section::Mail, Command::New) => self.mail.compose(),
            (Section::Mail, Command::Delete) => self.mail.trash(),
            (Section::Drive, Command::Refresh) => self.drive.refresh(),
            (Section::Drive, Command::Find) => self.drive.seek(),
            (Section::Drive, Command::Back) => self.drive.back(),
            (Section::Drive, Command::Forward) => self.drive.forward(),
            (Section::Drive, Command::Up) => self.drive.up(),
            (Section::Drive, Command::SelectAll) => self.drive.select_all(),
            (Section::Drive, Command::Delete) => self.drive.discard_selected(),
            (Section::Drive, Command::Rename) => self.drive.begin_rename(),
            (Section::Passwords, Command::Refresh) => {
                self.passwords
                    .sync(Some(google_passwords::Refresh::Trigger));
            }
            (Section::Passwords, Command::New) => self.passwords.add(),
            _ => skip_view(),
        }
    }

    pub(crate) fn toggle_account_menu(&mut self) {
        let open = !*self.account_menu;
        self.account_menu.set(open);
    }

    pub(crate) fn close_account_menu(&mut self) {
        self.account_menu.set(false);
    }

    pub(crate) fn dismiss_failure(&mut self) {
        self.failure.set(None);
    }
    pub(crate) fn sign_in(&mut self) {
        self.open_portal(Purpose::SignIn);
    }

    pub(crate) fn retrieve_vault_keys(&mut self) {
        self.open_portal(Purpose::VaultKeys);
    }

    pub(crate) fn cancel_portal(&mut self) {
        self.close_portal();
        self.stage.set(match self.session {
            Some(_) => Stage::Ready,
            None => Stage::Welcome,
        });
    }

    pub(crate) fn switch_to(&mut self, key: &str) {
        self.account_menu.set(false);
        if self
            .session
            .as_ref()
            .is_some_and(|session| session.key == key)
        {
            return;
        }
        if let Err(error) = self.accounts.choose(key) {
            self.failure.set(Some(error.to_string()));
            return;
        }
        self.start();
    }

    pub(crate) fn forget(&mut self, key: &str) {
        self.account_menu.set(false);
        let cache = self.roots.cache_of(key);
        match self.accounts.forget(key) {
            Ok(removal) => drop(removal),
            Err(error) => {
                self.failure.set(Some(error.to_string()));
                return;
            }
        }
        spawn_blocking(
            move || std::fs::remove_dir_all(cache),
            |removed: std::io::Result<()>| {
                if let Err(error) = removed
                    && error.kind() != std::io::ErrorKind::NotFound
                {
                    tracing::warn!("failed to remove the account cache: {error}");
                }
                skip_view();
            },
        );
        if self
            .session
            .as_ref()
            .is_some_and(|session| session.key == key)
        {
            self.leave();
            self.start();
        }
    }

    fn resume(&mut self, saved: SavedAccount) {
        self.leave();
        self.stage.set(Stage::Starting);
        let folder = self.accounts.folder(&saved.key);
        let cache = self.roots.cache_of(&saved.key);
        spawn_keyed(
            RESUME,
            Session::resume(saved, folder, cache),
            |google: &mut Google, resumed| google.resumed(resumed),
        );
    }

    fn resumed(&mut self, resumed: Result<Session, Failure>) {
        match resumed {
            Ok(session) => self.enter(session),
            Err(failure) => self.refuse(failure),
        }
    }

    fn enter(&mut self, session: Session) {
        let session = Arc::new(session);
        self.mail.attach(Arc::clone(&session));
        self.drive.attach(Arc::clone(&session));
        self.passwords.attach(Arc::clone(&session));
        self.session = Some(session);
        self.failure.set(None);
        self.stage.set(Stage::Ready);
        self.select(*self.section);
    }

    fn leave(&mut self) {
        self.session = None;
        self.mail.detach();
        self.drive.detach();
        self.passwords.detach();
    }

    fn refuse(&mut self, failure: Failure) {
        let reason = match failure {
            Failure::SignedOut => tr!("google.signing.expired").to_string(),
            Failure::Other(reason) => reason,
        };
        self.failure.set(Some(reason));
        self.stage.set(match self.session {
            Some(_) => Stage::Ready,
            None => Stage::Welcome,
        });
    }

    fn open_portal(&mut self, purpose: Purpose) {
        self.account_menu.set(false);
        let portal = match Portal::open(purpose) {
            Ok(portal) => portal,
            Err(reason) => {
                self.failure.set(Some(reason));
                return;
            }
        };
        if let Some(events) = portal.session().events() {
            stream_keyed(PORTAL, events, |google: &mut Google, event: PageEvent| {
                google.portal_heard(event);
            });
        }
        if purpose == Purpose::SignIn {
            every_keyed(TOKEN_WATCH, TOKEN_WATCH_PERIOD, Google::read_token);
        }
        self.portal = Some(portal);
        self.failure.set(None);
        self.stage.set(Stage::Page(purpose));
    }

    fn close_portal(&mut self) {
        cancel(PORTAL);
        cancel(TOKEN);
        cancel(TOKEN_WATCH);
        self.portal = None;
    }

    fn portal_heard(&mut self, event: PageEvent) {
        let Some(portal) = &self.portal else {
            skip_view();
            return;
        };
        match (event, portal.purpose()) {
            (PageEvent::NewSession(opening), _) => {
                portal.follow(opening);
                skip_view();
            }
            (PageEvent::Closed(_), _) => self.cancel_portal(),
            (PageEvent::Message(message), Purpose::VaultKeys) => {
                let keys = self
                    .session
                    .as_ref()
                    .and_then(|session| portal.vault_keys(&message, &session.profile.id));
                match keys {
                    Some(keys) => {
                        self.cancel_portal();
                        self.passwords.offer_vault_keys(&keys);
                    }
                    None => skip_view(),
                }
            }
            (PageEvent::Committed { .. } | PageEvent::Finished { .. }, Purpose::SignIn) => {
                self.read_token();
            }
            (_, Purpose::SignIn | Purpose::VaultKeys) => skip_view(),
        }
    }

    fn read_token(&mut self) {
        skip_view();
        let Some(portal) = &self.portal else {
            return;
        };
        spawn_keyed(
            TOKEN,
            portal.sign_in_token(),
            |google: &mut Google, token| google.token_read(token),
        );
    }

    fn token_read(&mut self, token: Result<Option<String>, String>) {
        match token {
            Ok(Some(token)) if self.portal.is_some() => {
                self.close_portal();
                self.stage.set(Stage::Connecting);
                spawn(identified(token), |google: &mut Google, identified| {
                    google.identified(identified);
                });
            }
            Ok(_) => skip_view(),
            Err(reason) => {
                tracing::warn!("failed to read the sign-in cookies: {reason}");
                skip_view();
            }
        }
    }

    fn identified(&mut self, identified: Result<(Master, Profile), Failure>) {
        let (master, profile) = match identified {
            Ok(identified) => identified,
            Err(failure) => return self.refuse(failure),
        };
        let account = match self.accounts.reserve() {
            Ok(account) => account,
            Err(error) => return self.refuse(Failure::other(error)),
        };
        let folder = account.folder().to_owned();
        let kept = master.clone();
        self.pending = Some(Pending {
            account,
            master,
            profile,
        });
        spawn(
            async move {
                SavedAccount::keep_master(&folder, &kept)
                    .await
                    .map_err(|error| error.to_string())
            },
            |google: &mut Google, kept| google.master_kept(kept),
        );
    }

    fn master_kept(&mut self, kept: Result<(), String>) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        if let Err(reason) = kept {
            return self.refuse(Failure::Other(reason));
        }
        let Pending {
            account,
            master,
            profile,
        } = pending;
        let saved = match self
            .accounts
            .commit(account, |key| SavedAccount { key, profile })
        {
            Ok(committed) => committed.saved,
            Err(error) => return self.refuse(Failure::other(error)),
        };
        let folder = self.accounts.folder(&saved.key);
        let cache = self.roots.cache_of(&saved.key);
        self.leave();
        spawn_keyed(
            RESUME,
            async move { Session::open(saved, &folder, cache, master).await },
            |google: &mut Google, resumed| google.resumed(resumed),
        );
    }
}
