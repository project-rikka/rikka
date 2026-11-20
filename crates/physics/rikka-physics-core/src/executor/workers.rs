use std::fmt;
use std::sync::Arc;

use super::{Executor, SerialExecutor, ThreadName, WorkerPool};
use crate::{Error, Result};

#[must_use]
pub struct Threads {
    count: usize,
    name: Option<ThreadName>,
}

impl fmt::Debug for Threads {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Threads")
            .field("count", &self.count)
            .field("named", &self.name.is_some())
            .finish()
    }
}

impl Threads {
    pub fn new(count: usize) -> Self {
        Self { count, name: None }
    }

    pub fn named(mut self, name: impl Fn(usize) -> String + 'static) -> Self {
        self.name = Some(Box::new(name));
        self
    }
}

#[must_use]
pub enum Workers {
    Threads(Threads),
    Executor(Arc<dyn Executor>),
}

impl fmt::Debug for Workers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Threads(threads) => threads.fmt(f),
            Self::Executor(executor) => f
                .debug_struct("Executor")
                .field("workers", &executor.workers())
                .finish(),
        }
    }
}

impl Default for Workers {
    fn default() -> Self {
        Self::Threads(Threads::new(1))
    }
}

impl From<usize> for Workers {
    fn from(count: usize) -> Self {
        Self::Threads(Threads::new(count))
    }
}

impl From<Threads> for Workers {
    fn from(threads: Threads) -> Self {
        Self::Threads(threads)
    }
}

impl From<Arc<dyn Executor>> for Workers {
    fn from(executor: Arc<dyn Executor>) -> Self {
        Self::Executor(executor)
    }
}

impl Workers {
    pub fn build(self) -> Result<Arc<dyn Executor>> {
        match self {
            Self::Executor(executor) => Ok(executor),
            Self::Threads(Threads { count: 0, .. }) => Err(Error::NoWorkers),
            Self::Threads(Threads { count: 1, .. }) => Ok(Arc::new(SerialExecutor)),
            Self::Threads(Threads { count, name }) => {
                let pool = WorkerPool::builder().workers(count);
                let pool = match name {
                    Some(name) => pool.thread_name(name),
                    None => pool,
                };
                Ok(Arc::new(pool.build()?))
            }
        }
    }
}
