use rikka_sync::Mutex;
use std::fmt;
use std::num::NonZero;
use std::sync::Arc;
use std::thread;

use super::shared::Shared;
use super::{CurrentWorker, WorkerPool};
use crate::executor::ThreadName;
use crate::{Error, Result};

#[must_use]
pub struct WorkerPoolBuilder {
    workers: Option<usize>,
    thread_name: ThreadName,
}

impl fmt::Debug for WorkerPoolBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkerPoolBuilder")
            .field("workers", &self.workers)
            .finish_non_exhaustive()
    }
}

impl Default for WorkerPoolBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerPoolBuilder {
    pub fn new() -> Self {
        Self {
            workers: None,
            thread_name: Box::new(|worker| format!("rikka-physics-{worker}")),
        }
    }

    pub fn workers(mut self, workers: usize) -> Self {
        self.workers = Some(workers);
        self
    }

    pub fn thread_name(mut self, name: impl Fn(usize) -> String + 'static) -> Self {
        self.thread_name = Box::new(name);
        self
    }

    pub fn build(self) -> Result<WorkerPool> {
        let workers = self
            .workers
            .unwrap_or_else(|| thread::available_parallelism().map_or(1, NonZero::get));
        if workers == 0 {
            return Err(Error::NoWorkers);
        }
        let mut pool = WorkerPool {
            shared: Arc::new(Shared::new(workers)),
            syncs: Mutex::new(Vec::new()),
            threads: Vec::with_capacity(workers - 1),
            workers,
        };
        for worker in 1..workers {
            let name = (self.thread_name)(worker);
            let shared = pool.shared.clone();
            let pool_id = pool.id();
            let index = worker as u32;
            let thread = thread::Builder::new()
                .name(name.clone())
                .spawn(move || {
                    let _current = CurrentWorker::enter(pool_id, index);
                    shared.serve(index);
                })
                .map_err(|source| Error::Spawn { name, source })?;
            pool.threads.push(thread);
        }
        Ok(pool)
    }
}
