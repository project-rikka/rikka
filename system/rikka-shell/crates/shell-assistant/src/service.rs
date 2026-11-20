use rikka_assist::{ASSISTANT, ASSISTANT_VERSION, OPEN_KIT, OpenRequest, is_kit};
use rikka_seam::{Call, Manifest, Method, MethodInfo, SeamError, Service, ServiceContext};
use shell_kit::{Host, Presence, PresenceService};

use crate::assistance::Assistance;

pub(crate) const CAPABILITY: &str = ASSISTANT;

pub(crate) use shell_kit::{CLOSE, OPEN};

const SUMMARY: &str = "The desktop's own assistant";

const SUBJECT: &str = "the assistant";

#[derive(Debug)]
pub(super) struct AssistantService {
    presence: PresenceService,
    assistance: Assistance,
}

impl AssistantService {
    #[must_use]
    pub(crate) fn new(assistance: Assistance) -> AssistantService {
        AssistantService {
            presence: PresenceService::new(CAPABILITY, SUMMARY, SUBJECT),
            assistance,
        }
    }

    #[must_use]
    pub(crate) fn presence(&self) -> Presence {
        self.presence.presence()
    }
}

impl Service<Host> for AssistantService {
    fn manifest(&self) -> Manifest {
        let presence = self.presence.manifest();
        let mut manifest = Manifest::new(CAPABILITY, ASSISTANT_VERSION).summary(SUMMARY);
        for method in presence.methods() {
            manifest = manifest.method(method.clone());
        }
        for event in presence.events() {
            manifest = manifest.event(event.clone());
        }
        manifest.method(
            MethodInfo::new(OPEN_KIT).summary("Opens the assistant with the app kit `kit` loaded"),
        )
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.presence.attach(seam);
    }

    fn call(&mut self, _: &mut Host, method: Method, call: Call) {
        if &*method != OPEN_KIT {
            return self.presence.answer(method, call);
        }
        match call.decode::<OpenRequest>() {
            Ok(request) if is_kit(request.kit()) => {
                self.assistance.summon(request.kit().to_owned());
                self.presence.answer(Method::from(OPEN), call);
            }
            Ok(request) => call.fail(SeamError::InvalidParams(format!(
                "\"{}\" names no app",
                request.kit()
            ))),
            Err(error) => call.fail(error),
        }
    }
}
