use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, OnceLock};

use rikka_sync::{Condvar, Mutex};

use crate::core::exit;

type Job = Box<dyn FnOnce() + Send>;

pub(crate) struct Pool {
    name: &'static str,
    workers: fn() -> usize,
    queue: OnceLock<Option<Sender<Job>>>,
}

impl Pool {
    pub(crate) const fn new(name: &'static str, workers: fn() -> usize) -> Self {
        Self {
            name,
            workers,
            queue: OnceLock::new(),
        }
    }

    pub(crate) fn enqueue(&self, job: impl FnOnce() + Send + 'static) {
        let job: Job = Box::new(job);
        let job = match self.queue.get_or_init(|| self.start()) {
            Some(queue) => match queue.send(job) {
                Ok(()) => return,
                Err(mpsc::SendError(job)) => job,
            },
            None => job,
        };
        job();
    }

    fn start(&self) -> Option<Sender<Job>> {
        let (sender, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        let workers = (self.workers)();
        let mut started = 0;
        exit::hold();
        for index in 0..workers {
            let receiver = Arc::clone(&receiver);
            let name = match workers {
                1 => self.name.to_owned(),
                _ => format!("{}-{index}", self.name),
            };
            let spawned = std::thread::Builder::new().name(name).spawn(move || {
                loop {
                    let job = receiver.lock().recv();
                    match job {
                        Ok(job) => exit::run(job),
                        Err(mpsc::RecvError) => return,
                    }
                }
            });
            match spawned {
                Ok(_) => started += 1,
                Err(error) => tracing::warn!("failed to start a {} thread: {error}", self.name),
            }
        }
        match started {
            0 => {
                tracing::warn!("no {} thread started, working inline", self.name);
                None
            }
            _ => Some(sender),
        }
    }
}

enum Stage<T> {
    Queued,
    Running,
    Done(T),
    Taken,
}

pub(crate) struct Slot<T> {
    stage: Mutex<Stage<T>>,
    settled: Condvar,
}

impl<T> Slot<T> {
    pub(crate) fn queued() -> Self {
        Self {
            stage: Mutex::new(Stage::Queued),
            settled: Condvar::new(),
        }
    }

    pub(crate) fn run(&self, work: impl FnOnce() -> T) -> bool {
        {
            let mut stage = self.stage.lock();
            if !matches!(*stage, Stage::Queued) {
                return false;
            }
            *stage = Stage::Running;
        }
        let value = work();
        *self.stage.lock() = Stage::Done(value);
        self.settled.notify_all();
        true
    }

    pub(crate) fn is_done(&self) -> bool {
        matches!(*self.stage.lock(), Stage::Done(_))
    }

    pub(crate) fn unless_done(&self, pending: impl FnOnce()) {
        let stage = self.stage.lock();
        if !matches!(*stage, Stage::Done(_)) {
            pending();
        }
    }

    pub(crate) fn take(&self) -> Option<T> {
        let mut stage = self.stage.lock();
        match &*stage {
            Stage::Done(_) => Self::taken(&mut stage),
            Stage::Queued | Stage::Running | Stage::Taken => None,
        }
    }

    pub(crate) fn claim(&self) -> Option<T> {
        let mut stage = self
            .settled
            .wait_while(self.stage.lock(), |stage| matches!(stage, Stage::Running));
        Self::taken(&mut stage)
    }

    fn taken(stage: &mut Stage<T>) -> Option<T> {
        match std::mem::replace(stage, Stage::Taken) {
            Stage::Done(value) => Some(value),
            Stage::Queued | Stage::Running | Stage::Taken => None,
        }
    }
}
