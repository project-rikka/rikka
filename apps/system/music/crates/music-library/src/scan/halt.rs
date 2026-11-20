use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Halt<'a> {
    epoch: &'a AtomicU64,
    at: u64,
}

impl Halt<'_> {
    pub(crate) fn new(epoch: &AtomicU64, at: u64) -> Halt<'_> {
        Halt { epoch, at }
    }

    pub(crate) fn is_set(self) -> bool {
        self.epoch.load(Ordering::SeqCst) != self.at
    }
}
