mod acting;
mod prompt;

pub(crate) use prompt::view as prompt_view;

use rikka_sync::Mutex;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_mime::MimeType;
use rikka_ui::Observers;
use rikka_worker::{Broadcast, Subscription};
use rikka_xdg::{MimeApps, Watch};

use browser_model::WindowId;

const APPLICATION: &str = "rikka-browser.desktop";
const LINKS: [&str; 2] = ["x-scheme-handler/http", "x-scheme-handler/https"];
const CLAIMED: [&str; 4] = [
    "x-scheme-handler/http",
    "x-scheme-handler/https",
    "text/html",
    "application/xhtml+xml",
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum DefaultStatus {
    #[default]
    Unchecked,
    Default,
    Other(Option<String>),
    Unavailable,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Attempt {
    #[default]
    Idle,
    Making,
    Failed(Failure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    ReadOnly(PathBuf),
    Refused(String),
}

#[derive(State, Debug)]
pub(crate) struct Claim {
    attempt: Tracked<Attempt>,
}

impl Claim {
    pub(crate) fn new() -> Claim {
        Claim {
            attempt: Tracked::new(Attempt::Idle),
        }
    }

    pub(crate) fn attempt(&self) -> Attempt {
        (*self.attempt).clone()
    }

    fn set(&mut self, attempt: Attempt) {
        if *self.attempt != attempt {
            self.attempt.set(attempt);
        }
    }

    fn forget_failure(&mut self) {
        if matches!(*self.attempt, Attempt::Failed(_)) {
            self.attempt.set(Attempt::Idle);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Prompt {
    #[default]
    Pending,
    Shown(WindowId),
    Finished,
}

#[derive(Default)]
struct Statuses {
    listeners: Broadcast<DefaultStatus>,
    turn: Mutex<()>,
}

impl Statuses {
    fn subscribe(&self) -> Subscription<DefaultStatus> {
        self.listeners.subscribe()
    }

    fn notify(&self, apps: &MimeApps) {
        let _turn = self.turn.lock();
        self.listeners.send(&status_in(apps));
    }
}

#[derive(Default)]
pub(crate) struct DefaultBrowser {
    status: RefCell<DefaultStatus>,
    prompt: Cell<Prompt>,
    observers: Observers,
    statuses: Arc<Statuses>,
    apps: RefCell<Option<MimeApps>>,
    watch: Arc<Mutex<Option<Watch>>>,
    begun: Cell<bool>,
}

impl DefaultBrowser {
    pub(crate) fn status(&self) -> DefaultStatus {
        self.observers.read();
        self.status.borrow().clone()
    }

    pub(crate) fn prompted(&self, window: WindowId) -> bool {
        self.observers.read();
        self.prompt.get() == Prompt::Shown(window)
    }

    fn apps(&self) -> Option<MimeApps> {
        self.apps.borrow().clone()
    }

    fn changes(&self) -> Subscription<DefaultStatus> {
        let changes = self.statuses.subscribe();
        self.begin();
        changes
    }

    fn begin(&self) {
        if self.begun.replace(true) {
            return;
        }
        let Some(apps) = MimeApps::session() else {
            self.learned(DefaultStatus::Unavailable);
            return;
        };
        let checking = apps.clone();
        let statuses = Arc::clone(&self.statuses);
        let watch_handle = Arc::clone(&self.watch);
        let started = std::thread::Builder::new()
            .name("rikka-browser-default".to_owned())
            .spawn(move || {
                let watching = checking.clone();
                let notifier = Arc::clone(&statuses);
                match checking.watch(move || notifier.notify(&watching)) {
                    Ok(watch) => *watch_handle.lock() = Some(watch),
                    Err(error) => {
                        tracing::warn!(%error, "cannot watch default browser setting");
                    }
                }
                statuses.notify(&checking);
            });
        if let Err(error) = started {
            tracing::warn!(%error, "cannot read default browser setting");
        }
        *self.apps.borrow_mut() = Some(apps);
    }

    fn learned(&self, status: DefaultStatus) {
        if *self.status.borrow() == status {
            return;
        }
        if !matches!(status, DefaultStatus::Other(_))
            && matches!(self.prompt.get(), Prompt::Shown(_))
        {
            self.prompt.set(Prompt::Finished);
        }
        *self.status.borrow_mut() = status;
        self.observers.changed();
    }

    fn offer(&self, window: WindowId, check_default: bool) {
        if self.prompt.get() != Prompt::Pending {
            return;
        }
        let next = match &*self.status.borrow() {
            DefaultStatus::Unchecked => return,
            DefaultStatus::Other(_) if check_default => Prompt::Shown(window),
            _ => Prompt::Finished,
        };
        self.prompt.set(next);
        self.observers.changed();
    }

    fn finish_prompt(&self) {
        if self.prompt.replace(Prompt::Finished) != Prompt::Finished {
            self.observers.changed();
        }
    }
}

impl std::fmt::Debug for DefaultBrowser {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DefaultBrowser")
            .field("status", &self.status)
            .field("prompt", &self.prompt)
            .finish_non_exhaustive()
    }
}

pub(crate) fn failure_text(failure: &Failure) -> Label {
    match failure {
        Failure::ReadOnly(list) => tr!(
            "browser.default.read-only",
            list = &list.display().to_string()
        ),
        Failure::Refused(reason) => tr!("browser.default.failed", reason = reason.as_str()),
    }
}

fn status_in(apps: &MimeApps) -> DefaultStatus {
    let associations = apps.read();
    if associations.application(APPLICATION).is_none() {
        return DefaultStatus::Unavailable;
    }
    for link in LINKS.map(MimeType::well_known) {
        match associations.default_for(&link) {
            Some(entry) if entry.id == APPLICATION => {}
            other => return DefaultStatus::Other(other.map(|app| app.name.clone())),
        }
    }
    DefaultStatus::Default
}

fn claim(apps: &MimeApps) -> Result<DefaultStatus, Failure> {
    apps.set_default(APPLICATION, &CLAIMED.map(MimeType::well_known))
        .map_err(|error| match error {
            rikka_xdg::Error::AssociationsReadOnly { list } => Failure::ReadOnly(list),
            other => Failure::Refused(other.to_string()),
        })?;
    Ok(status_in(apps))
}
