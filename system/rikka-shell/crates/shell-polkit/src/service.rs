use rikka_polkit::{Agent, Ask};
use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, Service, ServiceContext, Version};
use shell_kit::{Host, Presence};

pub(crate) const CAPABILITY: &str = "rikka.polkit";

pub(super) const PENDING: &str = "pending";

pub(crate) const REPORT: &str = "report";

pub(crate) const CHANGED: &str = "changed";

#[derive(Debug)]
pub(super) struct PolkitService {
    presence: Presence,
    asking: Option<Ask>,
    agent: Agent,
    seam: Option<ServiceContext>,
}

impl PolkitService {
    #[must_use]
    pub(crate) fn new(agent: Agent) -> PolkitService {
        PolkitService {
            presence: Presence::default(),
            asking: None,
            agent,
            seam: None,
        }
    }

    #[must_use]
    pub(crate) fn presence(&self) -> Presence {
        self.presence.clone()
    }

    #[must_use]
    pub(crate) fn agent(&self) -> Agent {
        self.agent.clone()
    }

    fn settle(&mut self, asking: Option<Box<Ask>>) {
        self.asking = asking.map(|ask| *ask);
        self.presence.set(self.asking.is_some());
        if let Some(seam) = &self.seam
            && let Err(error) = seam.publisher().emit(CHANGED, &self.asking)
        {
            tracing::warn!("failed to emit authentication dialog change: {error}");
        }
    }
}

impl Service<Host> for PolkitService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, Version::new(1, 0))
            .summary("Pending polkit authentication")
            .method(MethodInfo::new(PENDING).summary("Returns the pending request"))
            .method(MethodInfo::new(REPORT).summary("Agent status"))
            .event(
                EventInfo::new(CHANGED)
                    .summary("Pending request changed")
                    .latest(),
            )
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.seam = Some(seam);
    }

    fn call(&mut self, _: &mut Host, method: Method, call: Call) {
        match &*method {
            PENDING => call.encode(&self.asking),
            REPORT => match call.decode::<Option<Box<Ask>>>() {
                Ok(asking) => {
                    self.settle(asking);
                    call.done();
                }
                Err(error) => call.fail(error),
            },
            _ => call.unknown_method(),
        }
    }
}
