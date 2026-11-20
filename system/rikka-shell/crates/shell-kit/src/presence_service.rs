use rikka_seam::{
    Call, EventInfo, Manifest, Method, MethodInfo, SeamError, Service, ServiceContext, Version,
};

use crate::{Host, Presence};

pub const OPEN: &str = "open";

pub const CLOSE: &str = "close";

pub const TOGGLE: &str = "toggle";

pub(crate) const SHOWN: &str = "shown";

pub(crate) const CHANGED: &str = "changed";

#[derive(Debug)]
pub struct PresenceService {
    capability: &'static str,
    summary: &'static str,
    subject: &'static str,
    presence: Presence,
    seam: Option<ServiceContext>,
}

impl PresenceService {
    #[must_use]
    pub fn new(
        capability: &'static str,
        summary: &'static str,
        subject: &'static str,
    ) -> PresenceService {
        PresenceService {
            capability,
            summary,
            subject,
            presence: Presence::default(),
            seam: None,
        }
    }

    #[must_use]
    pub fn with_presence(mut self, presence: Presence) -> PresenceService {
        self.presence = presence;
        self
    }

    #[must_use]
    pub fn presence(&self) -> Presence {
        self.presence.clone()
    }

    fn set(&mut self, shown: bool) -> Result<(), SeamError> {
        if !self.presence.set(shown) {
            return Ok(());
        }
        match &self.seam {
            Some(seam) => seam.publisher().emit(CHANGED, &shown),
            None => Ok(()),
        }
    }

    fn settle(&mut self, shown: bool, call: Call) {
        match self.set(shown) {
            Ok(()) => call.encode(&shown),
            Err(error) => call.fail(error),
        }
    }

    pub fn hide(&mut self) {
        if let Err(error) = self.set(false) {
            tracing::warn!(%error, subject = self.subject, "failed to announce hiding");
        }
    }
}

impl Service<Host> for PresenceService {
    fn manifest(&self) -> Manifest {
        let subject = self.subject;
        Manifest::new(self.capability, Version::new(1, 0))
            .summary(self.summary)
            .method(MethodInfo::new(OPEN).summary(format!("Shows {subject}")))
            .method(MethodInfo::new(CLOSE).summary(format!("Hides {subject}")))
            .method(MethodInfo::new(TOGGLE).summary(format!("Toggles {subject}")))
            .method(MethodInfo::new(SHOWN).summary(format!("Returns whether {subject} is shown")))
            .event(
                EventInfo::new(CHANGED)
                    .summary(format!("Visibility of {subject} changed"))
                    .latest(),
            )
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.seam = Some(seam);
    }

    fn call(&mut self, _: &mut Host, method: Method, call: Call) {
        self.answer(method, call);
    }
}

impl PresenceService {
    pub fn answer(&mut self, method: Method, call: Call) {
        match &*method {
            OPEN => self.settle(true, call),
            CLOSE => self.settle(false, call),
            TOGGLE => {
                let show = !self.presence.is_shown();
                self.settle(show, call);
            }
            SHOWN => call.encode(&self.presence.is_shown()),
            _ => call.unknown_method(),
        }
    }
}
