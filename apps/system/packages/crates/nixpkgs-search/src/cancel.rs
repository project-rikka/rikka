use rikka_sync::{Mutex, MutexGuard};
use std::process::Child;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rikka_net::CancelToken;

#[derive(Clone, Debug, Default)]
pub struct Cancel(Arc<CancelState>);

#[derive(Debug, Default)]
struct CancelState {
    stopped: AtomicBool,
    running: Mutex<Option<Child>>,
    token: CancelToken,
}

impl Cancel {
    #[must_use]
    pub fn new() -> Cancel {
        Cancel::default()
    }

    pub fn stop(&self) {
        self.0.stopped.store(true, Ordering::SeqCst);
        self.0.token.cancel();
        if let Some(running) = self.child().as_mut() {
            let _ = running.kill();
        }
    }

    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.0.stopped.load(Ordering::SeqCst)
    }

    pub(crate) fn token(&self) -> CancelToken {
        self.0.token.clone()
    }

    pub(crate) fn track(&self, child: Child) {
        let mut running = self.child();
        *running = Some(child);
        if self.is_stopped()
            && let Some(running) = running.as_mut()
        {
            let _ = running.kill();
        }
    }

    pub(crate) fn take_child(&self) -> Option<Child> {
        self.child().take()
    }

    fn child(&self) -> MutexGuard<'_, Option<Child>> {
        self.0.running.lock()
    }
}
