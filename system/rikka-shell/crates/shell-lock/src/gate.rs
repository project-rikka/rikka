use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use futures_util::Stream;
use rikka_dirs::passwd::Entry;
use rikka_pam::{Authentication, Secret};
use rikka_sync::Mutex;
use rikka_ui::{Observers, TextDocument};
use rikka_worker::Observable;
use shell_kit::{Sealed, SessionLock};
use zeroize::Zeroizing;

use crate::keyring;
use crate::report::{AuthEvent, Report};

const SERVICE: &str = "rikka-lock";
const FALLBACK_SERVICE: &str = "login";
const SERVICES: &str = "/etc/pam.d";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PamQuestion {
    serial: u64,
    pub(crate) text: String,
    pub(crate) echo: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Notice {
    Said { error: bool, text: String },
    Refused,
    Broken,
}

type ReportQueue = Arc<Mutex<Vec<Report>>>;

#[derive(Clone)]
pub(super) struct Gate(Rc<Shared>);

struct Shared {
    lock: SessionLock,
    account: Option<Entry>,
    service: &'static str,
    typed: TextDocument,
    reports: ReportQueue,
    wake: Observable<()>,
    observers: Observers,
    inner: RefCell<Inner>,
}

#[derive(Default)]
struct Inner {
    talking: Option<Authentication>,
    talk: u64,
    question: Option<PamQuestion>,
    pending_secret: Option<Secret>,
    login_password: Option<Zeroizing<String>>,
    checking: bool,
    notice: Option<Notice>,
    refusals: u64,
}

impl Gate {
    #[must_use]
    pub(crate) fn new(lock: SessionLock) -> Gate {
        Gate(Rc::new(Shared {
            lock,
            account: rikka_dirs::passwd::current(),
            service: service(),
            typed: TextDocument::default(),
            reports: ReportQueue::default(),
            wake: Observable::default(),
            observers: Observers::new(),
            inner: RefCell::new(Inner::default()),
        }))
    }

    #[must_use]
    pub(crate) fn account(&self) -> Option<&Entry> {
        self.0.account.as_ref()
    }

    #[must_use]
    pub(crate) fn lock(&self) -> &SessionLock {
        &self.0.lock
    }

    #[must_use]
    pub(crate) fn typed(&self) -> &TextDocument {
        &self.0.typed
    }

    #[must_use]
    pub(crate) fn question(&self) -> Option<PamQuestion> {
        self.read(|inner| inner.question.clone())
    }

    #[must_use]
    pub(super) fn is_checking(&self) -> bool {
        self.read(|inner| inner.checking)
    }

    #[must_use]
    pub(crate) fn notice(&self) -> Option<Notice> {
        self.read(|inner| inner.notice.clone())
    }

    #[must_use]
    pub(super) fn refusals(&self) -> u64 {
        self.0.inner.borrow().refusals
    }

    pub(super) fn wakes(&self) -> impl Stream<Item = ()> + Send + 'static {
        self.0.wake.changes()
    }

    pub(super) fn drain(&self) {
        let reports = std::mem::take(&mut *self.0.reports.lock());
        for report in reports {
            self.event(report);
        }
    }

    pub(crate) fn begin(&self) {
        let silent = self.0.inner.borrow().talking.is_none();
        let sealing = self.0.lock.is_wanted() && self.0.lock.sealed() != Sealed::Open;
        if silent && sealing {
            self.talk();
        }
    }

    pub(crate) fn clear(&self) {
        self.0.typed.reset("");
    }

    pub(crate) fn end(&self) {
        self.clear();
        self.0.reports.lock().clear();
        let talking = self.write(|inner| {
            inner.question = None;
            inner.pending_secret = None;
            inner.login_password = None;
            inner.checking = false;
            inner.notice = None;
            inner.talk += 1;
            inner.talking.take()
        });
        drop(talking);
    }

    pub(crate) fn submit(&self) {
        let typed = self.0.typed.text();
        self.0.typed.reset("");
        if typed.is_empty() {
            return self.begin();
        }
        let kept = Zeroizing::new(typed.clone());
        let secret = Secret::new(typed);
        let question = self.write(|inner| {
            inner.checking = true;
            inner.notice = None;
            let hidden = inner
                .question
                .as_ref()
                .is_none_or(|question| !question.echo);
            if hidden && inner.login_password.is_none() {
                inner.login_password = Some(kept);
            }
            inner.question.take()
        });
        match question {
            Some(question) => self.answer(question.serial, secret),
            None => {
                let idle = self.write(|inner| {
                    inner.pending_secret = Some(secret);
                    inner.talking.is_none()
                });
                if idle {
                    self.talk();
                }
            }
        }
    }

    fn event(&self, report: Report) {
        if self.0.inner.borrow().talk != report.talk {
            return;
        }
        match report.event {
            AuthEvent::PamQuestion { serial, text, echo } => {
                let pending_secret = self.write(|inner| match inner.pending_secret.take() {
                    Some(secret) => Some(secret),
                    None => {
                        inner.checking = false;
                        inner.question = Some(PamQuestion { serial, text, echo });
                        None
                    }
                });
                if let Some(secret) = pending_secret {
                    self.answer(serial, secret);
                }
            }
            AuthEvent::Message { error, text } => {
                self.write(|inner| inner.notice = Some(Notice::Said { error, text }));
            }
            AuthEvent::Granted => self.open(),
            AuthEvent::Denied => {
                let answered = self.write(|inner| {
                    inner.talking = None;
                    inner.question = None;
                    inner.login_password = None;
                    let answered = std::mem::take(&mut inner.checking);
                    match answered {
                        true => {
                            inner.refusals += 1;
                            inner.notice = Some(Notice::Refused);
                        }
                        false => inner.notice = Some(Notice::Broken),
                    }
                    answered
                });
                if answered {
                    self.begin();
                }
            }
            AuthEvent::Cancelled => {
                self.write(|inner| {
                    inner.talking = None;
                    inner.checking = false;
                    inner.question = None;
                    inner.login_password = None;
                });
            }
            AuthEvent::Broken(reason) => {
                tracing::warn!("cannot check password: {reason}");
                self.write(|inner| {
                    inner.talking = None;
                    inner.checking = false;
                    inner.question = None;
                    inner.pending_secret = None;
                    inner.login_password = None;
                    inner.notice = Some(Notice::Broken);
                });
            }
        }
    }

    fn open(&self) {
        tracing::info!("screen unlocked");
        let login_password = self.write(|inner| {
            inner.talking = None;
            inner.pending_secret = None;
            inner.checking = false;
            inner.question = None;
            inner.login_password.take()
        });
        self.0.lock.unlock();
        if let Some(password) = login_password {
            keyring::log_in(password);
        }
    }

    fn answer(&self, serial: u64, secret: Secret) {
        if let Some(talking) = &self.0.inner.borrow().talking {
            talking.answer(serial, secret);
        }
    }

    fn talk(&self) {
        let Some(account) = &self.0.account else {
            tracing::error!("no user account to unlock the screen for");
            return self.write(|inner| {
                inner.checking = false;
                inner.notice = Some(Notice::Broken);
            });
        };
        let talk = self.write(|inner| {
            inner.talk += 1;
            inner.talking = None;
            inner.question = None;
            inner.talk
        });
        let (reports, wake) = (Arc::clone(&self.0.reports), self.0.wake.clone());
        let started = Authentication::start(self.0.service, &account.name, move |step| {
            reports.lock().push(Report {
                talk,
                event: AuthEvent::from(step),
            });
            wake.set(());
        });
        match started {
            Ok(talking) => self.write(|inner| inner.talking = Some(talking)),
            Err(error) => {
                tracing::error!("cannot check password: {error}");
                self.write(|inner| {
                    inner.checking = false;
                    inner.notice = Some(Notice::Broken);
                });
            }
        }
    }

    fn read<T>(&self, read: impl FnOnce(&Inner) -> T) -> T {
        self.0.observers.read();
        read(&self.0.inner.borrow())
    }

    fn write<T>(&self, write: impl FnOnce(&mut Inner) -> T) -> T {
        let result = write(&mut self.0.inner.borrow_mut());
        self.0.observers.changed();
        result
    }
}

fn service() -> &'static str {
    let ours = Path::new(SERVICES).join(SERVICE);
    if ours.is_file() {
        return SERVICE;
    }
    tracing::warn!(
        "{} missing, authenticating through `{FALLBACK_SERVICE}`; add \
         `security.pam.services.{SERVICE} = {{ }};` to the system configuration",
        ours.display()
    );
    FALLBACK_SERVICE
}

impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.0.inner.borrow();
        f.debug_struct("Gate")
            .field("talking", &inner.talking.is_some())
            .field("checking", &inner.checking)
            .finish_non_exhaustive()
    }
}
