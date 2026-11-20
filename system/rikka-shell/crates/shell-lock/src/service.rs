use rikka_seam::{Call, Manifest, Method, MethodInfo, Service, Version};
use shell_kit::{Host, Sealed, Slot};

use crate::gate::Gate;
use crate::sleep::SleepInhibitor;

pub(crate) const CAPABILITY: &str = "rikka.lock-screen";

pub(crate) const LOCK: &str = "lock";

pub(super) const SLEEP: &str = "sleep";

#[derive(Debug)]
pub(super) struct BeforeSleep {
    pub(super) locks: bool,
}

#[derive(Debug)]
pub(super) struct LockService {
    gate: Gate,
    sleep_inhibitor: SleepInhibitor,
    before_sleep: Slot<BeforeSleep>,
}

impl LockService {
    #[must_use]
    pub(crate) fn new(
        gate: Gate,
        sleep_inhibitor: SleepInhibitor,
        before_sleep: Slot<BeforeSleep>,
    ) -> LockService {
        LockService {
            gate,
            sleep_inhibitor,
            before_sleep,
        }
    }

    fn lock(&self) {
        if !self.gate.lock().is_wanted() {
            tracing::info!("locking the screen");
            self.gate.end();
        }
        self.gate.lock().lock();
        self.gate.begin();
    }

    fn before_sleep(&self, host: &Host) {
        self.gate.end();
        if !host.get(self.before_sleep).locks {
            self.gate.begin();
            return self.sleep_inhibitor.release();
        }
        self.lock();
        if self.gate.lock().sealed() == Sealed::Locked {
            self.sleep_inhibitor.release();
        }
    }
}

impl Service<Host> for LockService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, Version::new(1, 0))
            .summary("Session screen lock")
            .method(MethodInfo::new(LOCK).summary("Locks the session"))
            .method(MethodInfo::new(SLEEP).summary("System is about to suspend"))
    }

    fn call(&mut self, host: &mut Host, method: Method, call: Call) {
        match &*method {
            LOCK => self.lock(),
            SLEEP => self.before_sleep(host),
            _ => return call.unknown_method(),
        }
        call.done();
    }
}
