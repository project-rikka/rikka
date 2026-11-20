use rikka_sync::Mutex;
use std::sync::Arc;
use std::thread::JoinHandle;

use crate::error::{Error, Result};
use crate::handshake;

pub trait Stop {
    fn stop(&self);
}

impl<T: Stop + ?Sized> Stop for Arc<T> {
    fn stop(&self) {
        (**self).stop();
    }
}

pub struct Worker<H: Stop> {
    name: &'static str,
    handle: H,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl<H: Stop> Worker<H> {
    pub fn spawn<B, R, E>(name: &'static str, handle: H, body: B) -> Result<Worker<H>, E>
    where
        B: FnOnce() -> std::result::Result<R, E> + Send + 'static,
        R: FnOnce() + 'static,
        E: Send + 'static,
    {
        let (startup_sender, startup) = handshake::open();

        let thread = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || match body() {
                Ok(run) => {
                    startup_sender.report_running();
                    run();
                }
                Err(error) => startup_sender.report_failure(error),
            })
            .map_err(Error::Spawn)?;

        match startup.wait() {
            Ok(()) => Ok(Worker {
                name,
                handle,
                thread: Mutex::new(Some(thread)),
            }),
            Err(error) => {
                join(name, thread);
                Err(error)
            }
        }
    }

    #[must_use]
    pub fn handle(&self) -> &H {
        &self.handle
    }

    pub fn stop(&self) {
        self.handle.stop();
        let mut thread = self.thread.lock();
        if let Some(thread) = thread.take() {
            join(self.name, thread);
        }
    }
}

impl<H: Stop> Drop for Worker<H> {
    fn drop(&mut self) {
        self.stop();
    }
}

impl<H: Stop> std::fmt::Debug for Worker<H> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Worker")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

fn join(name: &str, thread: JoinHandle<()>) {
    if thread.join().is_err() {
        tracing::error!(thread = name, "worker thread panicked");
    }
}
