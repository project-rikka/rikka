use rikka_sync::Mutex;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rikka_worker::{EventFd, EventNotifier, Stop, Worker};

use crate::error::{Error, Result};
use crate::event::Event;
use crate::reach::Reach;
use crate::watches::Watches;

const NAME: &str = "rikka-watch";

pub struct Watcher {
    watches: Arc<Mutex<Watches>>,
    _worker: Worker<Halt>,
}

struct Halt {
    stopped: Arc<AtomicBool>,
    notifier: EventNotifier,
}

impl Stop for Halt {
    fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        if let Err(error) = self.notifier.notify() {
            tracing::debug!(%error, "failed to wake file watcher");
        }
    }
}

impl Watcher {
    pub fn spawn(sink: impl FnMut(Event) + Send + 'static) -> Result<Watcher> {
        let watches = Watches::new()?;
        let readable = readiness(&watches).map_err(Error::Open)?;
        let watches = Arc::new(Mutex::new(watches));
        let (notifier, wake) = rikka_worker::event_fd().map_err(Error::Spawn)?;
        let stopped = Arc::new(AtomicBool::new(false));
        let halt = Halt {
            stopped: Arc::clone(&stopped),
            notifier,
        };
        let reading = Arc::clone(&watches);
        let worker = Worker::spawn(NAME, halt, move || {
            Ok::<_, std::convert::Infallible>(move || {
                follow(&reading, &readable, &wake, &stopped, sink);
            })
        })
        .map_err(|error| match error {
            rikka_worker::Error::Spawn(error) => Error::Spawn(error),
            rikka_worker::Error::Startup(never) => match never {},
            rikka_worker::Error::Exited => Error::Spawn(std::io::Error::other(error)),
        })?;
        Ok(Watcher {
            watches,
            _worker: worker,
        })
    }

    pub fn watch(&self, path: &Path, reach: Reach) -> Result<()> {
        self.watches.lock().watch(path, reach)
    }

    pub fn unwatch(&self, path: &Path) {
        self.watches.lock().unwatch(path);
    }

    #[must_use]
    pub fn is_watching(&self, path: &Path) -> bool {
        self.watches.lock().is_watching(path)
    }
}

impl std::fmt::Debug for Watcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Watcher").finish_non_exhaustive()
    }
}

fn follow(
    watches: &Mutex<Watches>,
    readable: &Readiness,
    wake: &EventFd,
    stopped: &AtomicBool,
    mut sink: impl FnMut(Event),
) {
    loop {
        let ready = wait(wake, readable);
        if stopped.load(Ordering::Acquire) {
            return;
        }
        match ready {
            Ok(true) => {}
            Ok(false) => continue,
            Err(error) => return tracing::warn!(%error, "file watcher stopped"),
        }
        let events = match watches.lock().read() {
            Ok(events) => events,
            Err(error) => return tracing::warn!(%error, "file watcher stopped"),
        };
        events.into_iter().for_each(&mut sink);
    }
}

#[cfg(unix)]
type Readiness = std::os::fd::OwnedFd;

#[cfg(unix)]
fn readiness(watches: &Watches) -> io::Result<Readiness> {
    use std::os::fd::AsFd as _;
    watches.as_fd().try_clone_to_owned()
}

#[cfg(unix)]
fn wait(wake: &EventFd, readable: &Readiness) -> io::Result<bool> {
    use std::os::fd::AsFd as _;
    wake.wait_readable(readable.as_fd(), None)
}

#[cfg(windows)]
type Readiness = std::os::windows::io::OwnedHandle;

#[cfg(windows)]
fn readiness(watches: &Watches) -> io::Result<Readiness> {
    use std::os::windows::io::AsHandle as _;
    watches.as_handle().try_clone_to_owned()
}

#[cfg(windows)]
fn wait(wake: &EventFd, readable: &Readiness) -> io::Result<bool> {
    use std::os::windows::io::AsHandle as _;
    wake.wait_signaled([readable.as_handle()], None)
        .map(|[ready]| ready)
}
