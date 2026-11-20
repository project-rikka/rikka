use std::convert::Infallible;
use std::os::fd::AsFd as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rikka_wayland_client::Connection;
use rikka_worker::{EventFd, EventNotifier, Stop, Worker};

use crate::error::{Error, Result};

pub(crate) type Reader = Worker<StopReader>;

pub(crate) struct StopReader {
    stopped: Arc<AtomicBool>,
    notifier: EventNotifier,
}

impl Stop for StopReader {
    fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        if let Err(error) = self.notifier.notify() {
            tracing::debug!(%error, "failed to wake wayland reader");
        }
    }
}

pub(crate) fn start(connection: &Connection) -> Result<Reader> {
    let (notifier, events) = rikka_worker::event_fd()?;
    let stopped = Arc::new(AtomicBool::new(false));
    let stop = StopReader {
        stopped: Arc::clone(&stopped),
        notifier,
    };
    let connection = connection.clone();
    Worker::spawn("rikka-wayland", stop, move || {
        Ok::<_, Infallible>(move || read(&connection, &events, &stopped))
    })
    .map_err(|error| match error {
        rikka_worker::Error::Spawn(error) => Error::Io(error),
        rikka_worker::Error::Startup(never) => match never {},
        rikka_worker::Error::Exited => Error::Io(std::io::Error::other(error)),
    })
}

fn read(connection: &Connection, events: &EventFd, stopped: &AtomicBool) {
    while !stopped.load(Ordering::Acquire) {
        if let Err(error) = read_once(connection, events) {
            tracing::warn!(%error, "wayland reader stopped");
            return;
        }
    }
}

fn read_once(connection: &Connection, events: &EventFd) -> Result<()> {
    if !events.wait_readable(connection.as_fd(), None)? {
        events.drain();
        return Ok(());
    }
    connection.read()?;
    Ok(())
}
