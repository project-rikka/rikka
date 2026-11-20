mod builder;
mod current;
mod parking;
mod shared;

use rikka_sync::Mutex;
use std::fmt;
use std::panic;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub use builder::WorkerPoolBuilder;
use current::CurrentWorker;
use shared::{Shared, StageSync};

use super::{Block, Executor, Stage, run_serially};

pub struct WorkerPool {
    shared: Arc<Shared>,
    syncs: Mutex<Vec<StageSync>>,
    threads: Vec<JoinHandle<()>>,
    workers: usize,
}

impl fmt::Debug for WorkerPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkerPool")
            .field("workers", &self.workers)
            .finish()
    }
}

impl WorkerPool {
    pub fn builder() -> WorkerPoolBuilder {
        WorkerPoolBuilder::new()
    }

    fn id(&self) -> usize {
        Arc::as_ptr(&self.shared).addr()
    }
}

impl Executor for WorkerPool {
    fn workers(&self) -> usize {
        self.workers
    }

    fn run(&self, stages: &[Stage], task: &(dyn Fn(Block) + Sync)) {
        if let Some(worker) = CurrentWorker::index_in(self.id()) {
            run_serially(stages, task, worker);
            return;
        }
        let mut syncs = self.syncs.lock();
        let current = CurrentWorker::enter(self.id(), 0);
        if self.threads.is_empty() || stages.iter().all(|stage| stage.blocks <= 1) {
            run_serially(stages, task, 0);
            return;
        }
        if syncs.len() < stages.len() {
            syncs.resize_with(stages.len(), StageSync::default);
        }
        let panicked = self.shared.run(&syncs, stages, task);
        drop(current);
        drop(syncs);
        if let Some(payload) = panicked {
            panic::resume_unwind(payload);
        }
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        self.shared.shut_down();
        for worker in self.threads.drain(..) {
            if let Err(payload) = worker.join()
                && !thread::panicking()
            {
                panic::resume_unwind(payload);
            }
        }
    }
}
