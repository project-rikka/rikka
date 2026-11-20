use rikka_seam::{Call, Manifest, Method, MethodInfo, Service, Version};
use shell_kit::{Host, Slot};

use crate::policy::Policy;

pub(super) const CAPABILITY: &str = "rikka.power-saving";

pub(super) const REVIEW: &str = "review";

#[derive(Debug)]
pub(super) struct PowerSavingService {
    policy: Slot<Policy>,
}

impl PowerSavingService {
    pub(super) fn new(policy: Slot<Policy>) -> PowerSavingService {
        PowerSavingService { policy }
    }
}

impl Service<Host> for PowerSavingService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, Version::new(1, 0))
            .summary("Idle and battery power saving")
            .method(
                MethodInfo::new(REVIEW)
                    .summary("The battery or power profile changed; adjust timers and power saver"),
            )
    }

    fn call(&mut self, host: &mut Host, method: Method, call: Call) {
        match &*method {
            REVIEW => {
                host.get_mut(self.policy).review();
                call.done();
            }
            _ => call.unknown_method(),
        }
    }
}
