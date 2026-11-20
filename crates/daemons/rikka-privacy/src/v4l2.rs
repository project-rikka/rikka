use std::os::fd::AsFd as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rikka_watch::{Event, Reach, Watches};
use rikka_worker::{EventFd, EventNotifier, Stop, Worker};

use crate::devices;
use crate::user::User;

const NAME: &str = "rikka-privacy-cameras";
const DEV: &str = "/dev";
const DEBOUNCE: Duration = Duration::from_millis(150);
const IDLE: Duration = Duration::from_secs(3600);

type EventSink = Box<dyn Fn(Vec<User>) + Send>;

pub(crate) struct Ring {
    notifier: EventNotifier,
    stopped: Arc<AtomicBool>,
}

impl Stop for Ring {
    fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        let _ = self.notifier.notify();
    }
}

pub(crate) fn start(report: EventSink) -> Result<Worker<Ring>, String> {
    let (notifier, wake_fd) = rikka_worker::event_fd().map_err(|error| error.to_string())?;
    let stopped = Arc::new(AtomicBool::new(false));
    let stopping = Arc::clone(&stopped);
    Worker::spawn(NAME, Ring { notifier, stopped }, move || {
        let watching = Watching::open().map_err(|error| error.to_string())?;
        Ok(move || watching.run(&wake_fd, &stopping, &report))
    })
    .map_err(|error: rikka_worker::Error<String>| error.to_string())
}

struct Watching {
    watches: Watches,
    cameras: Vec<PathBuf>,
}

#[derive(Clone, Copy, Default)]
struct DeviceChanges {
    used: bool,
    replugged: bool,
}

impl DeviceChanges {
    fn merge(self, other: DeviceChanges) -> DeviceChanges {
        DeviceChanges {
            used: self.used || other.used,
            replugged: self.replugged || other.replugged,
        }
    }
}

impl Watching {
    fn open() -> rikka_watch::Result<Watching> {
        let mut watches = Watches::new()?;
        watches.watch(Path::new(DEV), Reach::Entries)?;
        let mut watching = Watching {
            watches,
            cameras: Vec::new(),
        };
        watching.rewatch();
        Ok(watching)
    }

    fn rewatch(&mut self) {
        self.cameras = devices::cameras();
        for camera in &self.cameras {
            if let Err(error) = self.watches.watch(camera, Reach::Uses) {
                tracing::debug!("{error}");
            }
        }
    }

    fn run(mut self, wake_fd: &EventFd, stopped: &AtomicBool, report: &EventSink) {
        report(devices::users(&self.cameras));
        loop {
            let readable = wake_fd.wait_readable(self.watches.as_fd(), IDLE);
            if stopped.load(Ordering::Acquire) {
                return;
            }
            match readable {
                Ok(true) => {}
                Ok(false) => continue,
                Err(error) => {
                    tracing::warn!("camera watch stopped: {error}");
                    return;
                }
            }
            let mut changes = self.drained();
            while wake_fd
                .wait_readable(self.watches.as_fd(), DEBOUNCE)
                .unwrap_or(false)
            {
                changes = changes.merge(self.drained());
            }
            if stopped.load(Ordering::Acquire) {
                return;
            }
            if changes.replugged {
                self.rewatch();
            }
            if changes.used || changes.replugged {
                report(devices::users(&self.cameras));
            }
        }
    }

    fn drained(&mut self) -> DeviceChanges {
        let mut changes = DeviceChanges::default();
        let events = self.watches.read().unwrap_or_else(|error| {
            tracing::debug!("{error}");
            vec![Event::Lost]
        });
        for event in events {
            match event {
                Event::Lost => {
                    changes.used = true;
                    changes.replugged = true;
                }
                Event::Changed(change) if change.kind.alters_listing() => {
                    changes.replugged |= change
                        .path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("video"));
                }
                Event::Changed(_) => changes.used = true,
            }
        }
        changes
    }
}
