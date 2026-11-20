use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, Service, ServiceContext};
use shell_kit::{Host, OPEN, Presence, PresenceService};

pub(crate) const CAPABILITY: &str = "rikka.translate";

pub(crate) const PICTURE: &str = "picture";

pub(super) const SNIPPED: &str = "snipped";

const SUMMARY: &str = "The desktop's translation window";

const SUBJECT: &str = "the translation window";

#[derive(Debug)]
pub(super) struct TranslateService {
    window: PresenceService,
    seam: Option<ServiceContext>,
}

impl TranslateService {
    #[must_use]
    pub(crate) fn new() -> TranslateService {
        TranslateService {
            window: PresenceService::new(CAPABILITY, SUMMARY, SUBJECT),
            seam: None,
        }
    }

    #[must_use]
    pub(crate) fn presence(&self) -> Presence {
        self.window.presence()
    }

    fn picture(&mut self, call: Call) {
        if let Some(seam) = &self.seam
            && let Err(error) = seam.publisher().emit(SNIPPED, &())
        {
            return call.fail(error);
        }
        self.window.answer(Method::from(OPEN), call);
    }
}

impl Service<Host> for TranslateService {
    fn manifest(&self) -> Manifest {
        self.window
            .manifest()
            .method(MethodInfo::new(PICTURE).summary("Translates the captured picture"))
            .event(EventInfo::new(SNIPPED).summary("Captured picture ready for translation"))
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.seam = Some(seam.clone());
        self.window.attach(seam);
    }

    fn call(&mut self, _: &mut Host, method: Method, call: Call) {
        match &*method {
            PICTURE => self.picture(call),
            settling => self.window.answer(Method::from(settling), call),
        }
    }
}
