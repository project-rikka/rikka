use rikka_sync::Mutex;
use std::sync::Arc;

use files_vfs::Reporter;
use rikka_types::Flow;

#[derive(Debug)]
pub(crate) struct Counter {
    reporter: Reporter,
    done: Mutex<Done>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Done {
    items: u64,
    bytes: u64,
}

impl Counter {
    pub(crate) fn new(reporter: &Reporter, items: u64, bytes: u64) -> Arc<Counter> {
        reporter.totals(items, bytes);
        Arc::new(Counter {
            reporter: reporter.clone(),
            done: Mutex::new(Done::default()),
        })
    }

    pub(crate) fn finished(&self, size: u64) {
        let mut done = self.done.lock();
        done.items += 1;
        done.bytes += size;
        self.reporter.reached(done.items, done.bytes);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.reporter.is_cancelled()
    }

    fn partly(&self, size: u64, fraction: f32) {
        let done = *self.done.lock();
        let part = (size as f64 * f64::from(fraction.clamp(0.0, 1.0))) as u64;
        self.reporter.reached(done.items, done.bytes + part);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Watch {
    counter: Arc<Counter>,
    size: u64,
}

impl Watch {
    pub(crate) fn new(counter: &Arc<Counter>, size: u64) -> Watch {
        Watch {
            counter: Arc::clone(counter),
            size,
        }
    }

    pub(crate) fn reached(&self, fraction: f32) -> Flow {
        if self.counter.is_cancelled() {
            return Flow::Cancel;
        }
        self.counter.partly(self.size, fraction);
        Flow::Continue
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.counter.is_cancelled()
    }
}
