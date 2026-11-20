use rikka_sync::{Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

#[derive(Debug, Default)]
pub(crate) struct OwnedThreads(Mutex<Vec<JoinHandle<()>>>);

impl OwnedThreads {
    pub(crate) fn adopt(&self, thread: JoinHandle<()>) {
        self.threads().push(thread);
    }

    pub(crate) fn take(&self) -> Vec<JoinHandle<()>> {
        std::mem::take(&mut *self.threads())
    }

    fn threads(&self) -> MutexGuard<'_, Vec<JoinHandle<()>>> {
        self.0.lock()
    }
}

pub(crate) fn join(thread: JoinHandle<()>) {
    if thread.thread().id() == thread::current().id() {
        return;
    }
    let name = thread.thread().name().unwrap_or("rikka-web").to_owned();
    if thread.join().is_err() {
        tracing::error!(thread = %name, "engine thread panicked");
    }
}
