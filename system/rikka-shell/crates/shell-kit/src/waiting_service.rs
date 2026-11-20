use std::fmt::Debug;

use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, Service, ServiceContext};
use serde::de::DeserializeOwned;

use crate::presence_service::OPEN;
use crate::{Host, Presence, PresenceService, Waiting};

pub const EDIT: &str = "edit";

pub const ARRIVED: &str = "arrived";

#[derive(Clone, Copy, Debug)]
pub struct WaitingDescriptor {
    pub capability: &'static str,
    pub summary: &'static str,
    pub subject: &'static str,
    pub edits: &'static str,
    pub arrives: &'static str,
}

#[derive(Debug)]
pub struct WaitingService<T> {
    awaited: WaitingDescriptor,
    window: PresenceService,
    waiting: Waiting<T>,
    seam: Option<ServiceContext>,
}

impl<T: DeserializeOwned> WaitingService<T> {
    #[must_use]
    pub fn new(awaited: WaitingDescriptor, waiting: Waiting<T>) -> WaitingService<T> {
        WaitingService {
            window: PresenceService::new(awaited.capability, awaited.summary, awaited.subject),
            awaited,
            waiting,
            seam: None,
        }
    }

    #[must_use]
    pub fn presence(&self) -> Presence {
        self.window.presence()
    }

    fn edit(&mut self, call: Call) {
        let opening = match call.decode::<T>() {
            Ok(opening) => opening,
            Err(error) => return call.fail(error),
        };
        self.waiting.deliver(opening);
        if let Some(seam) = &self.seam
            && let Err(error) = seam.publisher().emit(ARRIVED, &())
        {
            return call.fail(error);
        }
        self.window.answer(Method::from(OPEN), call);
    }

    fn answer(&mut self, method: Method, call: Call) {
        match &*method {
            EDIT => self.edit(call),
            settling => self.window.answer(Method::from(settling), call),
        }
    }
}

impl<T: DeserializeOwned + Debug + 'static> Service<Host> for WaitingService<T> {
    fn manifest(&self) -> Manifest {
        self.window
            .manifest()
            .method(MethodInfo::new(EDIT).summary(self.awaited.edits))
            .event(EventInfo::new(ARRIVED).summary(self.awaited.arrives))
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.seam = Some(seam.clone());
        self.window.attach(seam);
    }

    fn call(&mut self, _: &mut Host, method: Method, call: Call) {
        self.answer(method, call);
    }
}
