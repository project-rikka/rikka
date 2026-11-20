use rikka_sync::Mutex;
use std::io;

use rikka_worker::Stop;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;

pub(super) fn runtime() -> io::Result<Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
}

pub(super) struct Hangup(Mutex<Option<oneshot::Sender<()>>>);

impl Hangup {
    pub(super) fn new(stop: oneshot::Sender<()>) -> Hangup {
        Hangup(Mutex::new(Some(stop)))
    }
}

impl Stop for Hangup {
    fn stop(&self) {
        drop(self.0.lock().take());
    }
}

pub(super) fn started(error: rikka_worker::Error<io::Error>) -> io::Error {
    match error {
        rikka_worker::Error::Spawn(error) | rikka_worker::Error::Startup(error) => error,
        rikka_worker::Error::Exited => io::Error::other("the seam thread ended before it started"),
    }
}
