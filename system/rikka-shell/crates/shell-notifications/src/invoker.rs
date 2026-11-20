use std::collections::HashMap;
use std::sync::Arc;

use rikka_notifications::CloseReason;
use rikka_sync::{Mutex, MutexGuard};

#[derive(Clone, Copy, Debug, Default)]
struct Pending {
    under_way: u32,
    closing: Option<CloseReason>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ActionInvoker {
    pending: Arc<Mutex<HashMap<u32, Pending>>>,
}

impl ActionInvoker {
    pub(crate) fn begin(&self, id: u32) {
        self.entries().entry(id).or_default().under_way += 1;
    }

    #[must_use]
    pub(super) fn defers(&self, id: u32, reason: CloseReason) -> bool {
        let mut entries = self.entries();
        let Some(pending) = entries.get_mut(&id) else {
            return false;
        };
        pending.closing.get_or_insert(reason);
        true
    }

    #[must_use]
    pub(crate) fn finish(&self, id: u32) -> Option<CloseReason> {
        let mut entries = self.entries();
        let pending = entries.get_mut(&id)?;
        pending.under_way = pending.under_way.saturating_sub(1);
        if pending.under_way > 0 {
            return None;
        }
        entries.remove(&id).and_then(|pending| pending.closing)
    }

    fn entries(&self) -> MutexGuard<'_, HashMap<u32, Pending>> {
        self.pending.lock()
    }
}
