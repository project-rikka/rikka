use std::cell::Cell;
use std::rc::Rc;

use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, Service, ServiceContext, Version};
use serde::{Deserialize, Serialize};
use shell_kit::{Host, Presence, Sealed, SessionLock};

pub(crate) const CAPABILITY: &str = "rikka.keyring-prompt";

pub(super) const REPORT: &str = "report";

pub(super) const PENDING: &str = "pending";

pub(super) const CHANGED: &str = "changed";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Report {
    pub(crate) asking: bool,
}

#[derive(Clone, Debug)]
pub(super) struct Showing {
    presence: Presence,
    asking: Rc<Cell<bool>>,
    lock: SessionLock,
}

impl Showing {
    pub(super) fn new(lock: SessionLock) -> Showing {
        Showing {
            presence: Presence::default(),
            asking: Rc::default(),
            lock,
        }
    }

    pub(super) fn presence(&self) -> Presence {
        self.presence.clone()
    }

    pub(super) fn settle(&self) {
        self.presence
            .set(self.asking.get() && self.lock.sealed() == Sealed::Open);
    }

    fn ask(&self, asking: bool) {
        self.asking.set(asking);
        self.settle();
    }
}

#[derive(Debug)]
pub(super) struct KeyringService {
    showing: Showing,
    report: Report,
    seam: Option<ServiceContext>,
}

impl KeyringService {
    pub(super) fn new(showing: Showing) -> KeyringService {
        KeyringService {
            showing,
            report: Report::default(),
            seam: None,
        }
    }

    fn settle(&mut self, report: Report) {
        self.report = report;
        self.showing.ask(report.asking);
        if let Some(seam) = &self.seam
            && let Err(error) = seam.publisher().emit(CHANGED, &self.report)
        {
            tracing::warn!("failed to emit keyring prompt change: {error}");
        }
    }
}

impl Service<Host> for KeyringService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, Version::new(1, 0))
            .summary("Keyring prompts waiting for the person")
            .method(MethodInfo::new(PENDING).summary("Returns whether a prompt is waiting"))
            .method(MethodInfo::new(REPORT).summary("Keyring watcher status"))
            .event(
                EventInfo::new(CHANGED)
                    .summary("Waiting prompts changed")
                    .latest(),
            )
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.seam = Some(seam);
    }

    fn call(&mut self, _: &mut Host, method: Method, call: Call) {
        match &*method {
            PENDING => call.encode(&self.report),
            REPORT => match call.decode::<Report>() {
                Ok(report) => {
                    self.settle(report);
                    call.done();
                }
                Err(error) => call.fail(error),
            },
            _ => call.unknown_method(),
        }
    }
}
