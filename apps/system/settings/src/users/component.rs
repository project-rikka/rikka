use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;

use rikka_accounts::{Account, AccountKind, Accounts};
use rikka_app::prelude::*;
use rikka_dirs::passwd::Entry;
use rikka_nix::Module;
use rikka_widgets::column;

use crate::following::following;
use crate::page::Page;
use crate::pending::Pending;
use crate::session::Session;
use crate::users::cropping::Cropping;
use crate::users::password::PasswordForm;
use crate::users::{cropping, name, password, profile};

const CONNECTING: &str = "users-connecting";
const FOLLOWING: &str = "users-following";
const READING: &str = "users-reading";
const FACE: &str = "users-face";

#[derive(Debug)]
pub(crate) enum Link {
    Connecting,
    Ready(Accounts),
    Missing,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Report {
    PasswordChanged,
    KeyringBehind(String),
    PictureUnreadable(String),
    PictureRefused(String),
    AccountsLost,
}

pub(crate) struct Users {
    open: bool,
    who: Option<Entry>,
    link: Link,
    account: Option<Account>,
    face: Option<Arc<[u8]>>,
    name: TextDocument,
    pub(crate) cropping: Option<Cropping>,
    pub(crate) password: Option<PasswordForm>,
    pub(crate) report: Option<Report>,
    pub(crate) busy: bool,
}

impl Page for Users {
    fn opened(&mut self, pending: &Pending, _: &Session) {
        self.open = true;
        self.who = rikka_dirs::passwd::current();
        self.reread_name(pending);
        self.connect();
    }

    fn closed(&mut self) {
        self.open = false;
        for key in [CONNECTING, FOLLOWING, READING, FACE] {
            cancel(key);
        }
        self.cropping = None;
        if !self
            .password
            .as_ref()
            .is_some_and(PasswordForm::is_changing)
        {
            self.close_password();
        }
    }
}

impl Users {
    pub(crate) fn new() -> Self {
        Self {
            open: false,
            who: None,
            link: Link::Connecting,
            account: None,
            face: None,
            name: TextDocument::new(""),
            cropping: None,
            password: None,
            report: None,
            busy: false,
        }
    }

    pub(crate) fn rebuild_finished(&mut self, pending: &Pending) {
        if !self.open {
            return;
        }
        self.who = rikka_dirs::passwd::current();
        self.reread_name(pending);
        if matches!(self.link, Link::Missing | Link::Failed(_)) {
            self.connect();
        }
    }

    pub(crate) fn commit_name(&self, pending: &mut Pending) {
        let Some(who) = &self.who else {
            return;
        };
        name::write(
            pending,
            &who.name,
            &self.name.text(),
            who.full_name().unwrap_or_default(),
        );
    }

    pub(crate) fn link(&self) -> &Link {
        &self.link
    }

    pub(crate) fn accounts(&self) -> Option<&Accounts> {
        match &self.link {
            Link::Ready(accounts) => Some(accounts),
            _ => None,
        }
    }

    pub(crate) fn name(&self) -> &TextDocument {
        &self.name
    }

    pub(crate) fn who(&self) -> Option<&Entry> {
        self.who.as_ref()
    }

    pub(crate) fn kind(&self) -> Option<AccountKind> {
        self.account.as_ref().map(|account| account.kind)
    }

    pub(crate) fn face(&self) -> Option<ImageSource> {
        self.face.clone().map(ImageSource::Bytes)
    }

    pub(crate) fn has_picture(&self) -> bool {
        self.face.is_some()
    }

    pub(crate) fn dismiss(&mut self) {
        self.report = None;
    }

    pub(crate) fn read(&mut self) {
        let Some(accounts) = self.accounts().cloned() else {
            return;
        };
        spawn_keyed(
            READING,
            async move { accounts.account().await },
            |users: &mut Users, read: rikka_accounts::Result<Account>| match read {
                Ok(account) => users.show(account),
                Err(error) => users.fail(error.to_string()),
            },
        );
    }

    fn fail(&mut self, reason: String) {
        cancel(FOLLOWING);
        self.link = Link::Failed(reason);
    }

    fn reread_name(&mut self, pending: &Pending) {
        let Some(who) = &self.who else {
            self.name.reset("");
            return;
        };
        let shown = pending
            .edited(Module::System)
            .and_then(|document| name::staged(document, &who.name))
            .unwrap_or_else(|| who.full_name().unwrap_or_default().to_owned());
        self.name = TextDocument::new(&shown);
    }

    fn connect(&mut self) {
        if self.accounts().is_some() {
            self.follow();
            return;
        }
        self.link = Link::Connecting;
        spawn_keyed(
            CONNECTING,
            Accounts::current(),
            |users: &mut Users, linked: rikka_accounts::Result<Accounts>| {
                users.link = match linked {
                    Ok(accounts) => Link::Ready(accounts),
                    Err(rikka_accounts::Error::Missing) => Link::Missing,
                    Err(error) => Link::Failed(error.to_string()),
                };
                if users.open {
                    users.follow();
                }
            },
        );
    }

    fn follow(&mut self) {
        let Some(accounts) = self.accounts().cloned() else {
            return;
        };
        self.read();
        stream_keyed(
            FOLLOWING,
            following(async move { accounts.changes().await }),
            |users: &mut Users, change: Result<(), String>| match change {
                Ok(()) => users.read(),
                Err(reason) => users.fail(reason),
            },
        );
    }

    fn show(&mut self, account: Account) {
        let icon = account.icon.clone();
        self.account = Some(account);
        let Some(icon) = icon else {
            cancel(FACE);
            self.face = None;
            return;
        };
        spawn_blocking_keyed(
            FACE,
            move || match std::fs::read(&icon) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
                Err(error) => Err((icon, error.to_string())),
            },
            |users: &mut Users, read: Result<Option<Vec<u8>>, (PathBuf, String)>| match read {
                Ok(Some(bytes)) if users.face.as_deref() == Some(bytes.as_slice()) => skip_view(),
                Ok(bytes) => users.face = bytes.map(Arc::from),
                Err((icon, reason)) => {
                    users.face = None;
                    users.report = Some(Report::PictureUnreadable(format!(
                        "{}: {reason}",
                        icon.display()
                    )));
                }
            },
        );
    }
}

pub(crate) fn view(users: &Users, pending: &Pending) -> impl View {
    column! {
        profile::link_notice(users, pending);
        users.report.as_ref().map(profile::report_notice);
        match &users.cropping {
            Some(cropping) => cropping::view(cropping).build(),
            None => column! {
                profile::view(users);
                if pending.unavailable().is_none() {
                    name::view(users);
                }
                password::view(users.password.as_ref());
            }
            .gap(Space::ROOMY)
            .fill_width()
            .build(),
        }
    }
    .gap(Space::ROOMY)
}

impl std::fmt::Debug for Users {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Users")
            .field("link", &self.link)
            .field("account", &self.account)
            .finish_non_exhaustive()
    }
}
