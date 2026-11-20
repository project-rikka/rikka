use rikka_sync::{Condvar, Mutex, MutexGuard};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rustc_hash::FxHashSet;
use smallvec::SmallVec;

use crate::paint::measure_key;
use crate::task::{Shared, helpers, most_hands};
use crate::text::TextRun;

const WORTH_SHARING: usize = 12;

#[derive(Debug)]
pub(crate) struct ShapeRequest {
    pub(crate) run: TextRun,
    pub(crate) widths: SmallVec<[Option<f32>; 2]>,
}

pub(crate) fn warm(mut requests: Vec<ShapeRequest>) {
    let mut unique = FxHashSet::default();
    requests.retain(|piece| unique.insert(measure_key(&piece.run, None)));
    let lays: usize = requests.iter().map(|piece| piece.widths.len()).sum();
    if lays < WORTH_SHARING {
        return;
    }
    let hands = (lays / WORTH_SHARING).min(most_hands());
    let batch = Arc::new(Batch::new(requests));
    let shared: Arc<dyn Shared> = batch.clone();
    let helpers = helpers();
    helpers.distribute(&shared, hands);
    batch.work_off();
    batch.wait();
    helpers.withdraw(&shared);
}

#[derive(Debug)]
struct Batch {
    requests: Vec<ShapeRequest>,
    next: AtomicUsize,
    left: AtomicUsize,
    done: Mutex<bool>,
    finished: Condvar,
}

impl Batch {
    fn new(requests: Vec<ShapeRequest>) -> Batch {
        Batch {
            left: AtomicUsize::new(requests.len()),
            next: AtomicUsize::new(0),
            requests,
            done: Mutex::new(false),
            finished: Condvar::new(),
        }
    }

    fn did_one(&self) {
        if self.left.fetch_sub(1, Ordering::AcqRel) != 1 {
            return;
        }
        let mut done = self.done();
        *done = true;
        self.finished.notify_all();
    }

    fn done(&self) -> MutexGuard<'_, bool> {
        self.done.lock()
    }

    fn wait(&self) {
        let mut done = self.done();
        while !*done && self.left.load(Ordering::Acquire) > 0 {
            done = self.finished.wait(done);
        }
    }
}

impl Shared for Batch {
    fn work_off(&self) {
        loop {
            let index = self.next.fetch_add(1, Ordering::Relaxed);
            let Some(piece) = self.requests.get(index) else {
                return;
            };
            let counted = CompletionGuard(self);
            for &width in &piece.widths {
                let _ = rikka_text::layout(piece.run.as_shape(), width);
            }
            drop(counted);
        }
    }
}

struct CompletionGuard<'a>(&'a Batch);

impl Drop for CompletionGuard<'_> {
    fn drop(&mut self) {
        self.0.did_one();
    }
}
