use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
pub(crate) struct RunGuard(Arc<AtomicBool>);

impl RunGuard {
    pub(crate) fn try_acquire(running: &Arc<AtomicBool>) -> Option<RunGuard> {
        running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| RunGuard(Arc::clone(running)))
    }
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
