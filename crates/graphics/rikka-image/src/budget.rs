use std::sync::Arc;

use rikka_sync::{Condvar, Mutex};

pub const DEFAULT_BUDGET: usize = 128 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Budget {
    shared: Arc<Shared>,
}

#[derive(Debug)]
struct Shared {
    limit: usize,
    usage: Mutex<Usage>,
    freed: Condvar,
}

#[derive(Debug, Default)]
struct Usage {
    reserved: usize,
    closed: bool,
}

impl Budget {
    #[must_use]
    pub fn new(limit: usize) -> Budget {
        Budget {
            shared: Arc::new(Shared {
                limit: limit.max(1),
                usage: Mutex::new(Usage::default()),
                freed: Condvar::new(),
            }),
        }
    }

    #[must_use]
    pub fn reserved(&self) -> usize {
        self.shared.usage.lock().reserved
    }

    pub fn reserve(&self, bytes: usize) -> Reservation {
        let mut usage = self
            .shared
            .freed
            .wait_while(self.shared.usage.lock(), |usage| {
                !usage.closed
                    && usage.reserved > 0
                    && usage.reserved.saturating_add(bytes) > self.shared.limit
            });
        usage.reserved += bytes;
        drop(usage);
        Reservation {
            shared: Some(Arc::clone(&self.shared)),
            bytes,
        }
    }

    pub fn close(&self) {
        self.shared.usage.lock().closed = true;
        self.shared.freed.notify_all();
    }

    pub fn reserve_now(&self, bytes: usize) -> Reservation {
        self.shared.usage.lock().reserved += bytes;
        Reservation {
            shared: Some(Arc::clone(&self.shared)),
            bytes,
        }
    }
}

impl Default for Budget {
    fn default() -> Budget {
        Budget::new(DEFAULT_BUDGET)
    }
}

#[derive(Debug)]
#[must_use = "dropping the reservation releases its bytes immediately"]
pub struct Reservation {
    shared: Option<Arc<Shared>>,
    bytes: usize,
}

impl Reservation {
    pub fn empty() -> Reservation {
        Reservation {
            shared: None,
            bytes: 0,
        }
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        let Some(shared) = self.shared.take() else {
            return;
        };
        let mut usage = shared.usage.lock();
        usage.reserved = usage.reserved.saturating_sub(self.bytes);
        drop(usage);
        shared.freed.notify_all();
    }
}
