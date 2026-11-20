use rikka_sync::{Condvar, Mutex};
use rikka_worker::{Stop, Worker};
use std::convert::Infallible;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, OnceLock};

use crate::error::{Error, Result};

type Dropped = Box<dyn Send>;

struct Reaper {
    stopping: Mutex<Option<Sender<Dropped>>>,
    pending: Arc<Pending>,
}

#[derive(Default)]
struct Pending {
    count: Mutex<usize>,
    drained: Condvar,
}

impl Pending {
    fn add(&self) {
        *self.count.lock() += 1;
    }

    fn finish(&self) {
        let mut count = self.count.lock();
        *count = count.saturating_sub(1);
        if *count == 0 {
            self.drained.notify_all();
        }
    }

    fn wait(&self) {
        let mut count = self.count.lock();
        while *count > 0 {
            count = self.drained.wait(count);
        }
    }
}

static REAPER: OnceLock<Reaper> = OnceLock::new();

fn reaper() -> &'static Reaper {
    REAPER.get_or_init(|| {
        let (stopping, waiting) = channel::<Dropped>();
        let pending = Arc::new(Pending::default());
        let reaped = Arc::clone(&pending);
        let started = std::thread::Builder::new()
            .name("rikka-video-reaper".into())
            .spawn(move || {
                while let Ok(worker) = waiting.recv() {
                    drop(worker);
                    reaped.finish();
                }
            });
        match started {
            Ok(_) => Reaper {
                stopping: Mutex::new(Some(stopping)),
                pending,
            },
            Err(err) => {
                tracing::warn!(
                    "failed to spawn reaper thread, decoders will be joined on close: {err}"
                );
                Reaper {
                    stopping: Mutex::new(None),
                    pending,
                }
            }
        }
    })
}

#[derive(Debug)]
pub struct Uninterruptible;

impl Stop for Uninterruptible {
    fn stop(&self) {}
}

pub struct Reaped<H: Stop + Send + 'static> {
    worker: Option<Worker<H>>,
}

impl<H: Stop + Send + 'static> Drop for Reaped<H> {
    fn drop(&mut self) {
        let Some(worker) = self.worker.take() else {
            return;
        };
        worker.handle().stop();
        let reaper = reaper();
        reaper.pending.add();
        let handed = match reaper.stopping.lock().as_ref() {
            Some(stopping) => stopping.send(Box::new(worker)),
            None => Err(std::sync::mpsc::SendError(Box::new(worker) as Dropped)),
        };
        if let Err(refused) = handed {
            drop(refused.0);
            reaper.pending.finish();
        }
    }
}

impl<H: Stop + Send + 'static> std::fmt::Debug for Reaped<H> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Reaped").finish_non_exhaustive()
    }
}

pub fn spawn<H: Stop + Send + 'static>(
    name: &'static str,
    handle: H,
    body: impl FnOnce() + Send + 'static,
) -> Result<Reaped<H>> {
    let worker = Worker::spawn(name, handle, move || Ok::<_, Infallible>(body))
        .map_err(|error| Error::Worker(format!("cannot start {name}: {error}")))?;
    Ok(Reaped {
        worker: Some(worker),
    })
}

pub fn wait_for_closed_players() {
    let Some(reaper) = REAPER.get() else {
        return;
    };
    reaper.pending.wait();
}
