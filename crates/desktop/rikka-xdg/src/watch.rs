use std::convert::Infallible;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rikka_watch::{Event, Reach, Watches};
use rikka_worker::{EventFd, EventNotifier, Stop, Worker};

use crate::catalog::application_dirs;
use crate::error::{Error, Result};

const NAME: &str = "rikka-xdg-watch";
const SETTLED: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub struct Watch {
    _worker: Worker<Halt>,
}

#[derive(Debug)]
struct Halt {
    notifier: EventNotifier,
    stopped: Arc<AtomicBool>,
}

impl Stop for Halt {
    fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        let _ = self.notifier.notify();
    }
}

impl Watch {
    pub fn applications(changed: impl Fn() + Send + 'static) -> Result<Watch> {
        Watch::spawn(application_dirs(), changed)
    }

    pub fn folder(folder: &Path, changed: impl Fn() + Send + 'static) -> Result<Watch> {
        Watch::spawn(vec![folder.to_path_buf()], changed)
    }

    pub fn paths(paths: Vec<PathBuf>, changed: impl Fn() + Send + 'static) -> Result<Watch> {
        Watch::spawn(paths, changed)
    }

    pub(crate) fn spawn(roots: Vec<PathBuf>, changed: impl Fn() + Send + 'static) -> Result<Watch> {
        let mut watching = Watching {
            watches: fresh()?,
            roots,
        };
        watching.arm();

        let (notifier, wake_fd) = rikka_worker::event_fd().map_err(Error::Spawn)?;
        let stopped = Arc::new(AtomicBool::new(false));
        let halt = Halt {
            notifier,
            stopped: Arc::clone(&stopped),
        };
        let worker = Worker::spawn(NAME, halt, move || {
            Ok::<_, Infallible>(move || watching.follow(&wake_fd, &stopped, &changed))
        })
        .map_err(|error| match error {
            rikka_worker::Error::Spawn(error) => Error::Spawn(error),
            rikka_worker::Error::Startup(never) => match never {},
            rikka_worker::Error::Exited => Error::Spawn(std::io::Error::other(error)),
        })?;
        Ok(Watch { _worker: worker })
    }
}

struct Watching {
    watches: Watches,
    roots: Vec<PathBuf>,
}

impl Watching {
    fn follow(mut self, wake_fd: &EventFd, stopped: &AtomicBool, changed: &impl Fn()) {
        loop {
            let readable = ready(wake_fd, &self.watches, None);
            if stopped.load(Ordering::Acquire) {
                return;
            }
            match readable {
                Ok(true) => {}
                Ok(false) => continue,
                Err(error) => {
                    tracing::warn!(%error, "application directory watch stopped");
                    return;
                }
            }
            if !self.drain_events() {
                continue;
            }
            while ready(wake_fd, &self.watches, Some(SETTLED)).unwrap_or(false) {
                self.drain_events();
            }
            if stopped.load(Ordering::Acquire) {
                return;
            }
            self.arm();
            changed();
        }
    }

    fn arm(&mut self) {
        match fresh() {
            Ok(watches) => self.watches = watches,
            Err(error) => tracing::debug!(%error, "failed to rescan watched directories"),
        }
        for (path, reach) in watch_points(&self.roots) {
            if let Err(error) = self.watches.watch(&path, reach) {
                tracing::debug!(%error, "failed to watch directory");
            }
        }
    }

    fn drain_events(&mut self) -> bool {
        let events = match self.watches.read() {
            Ok(events) => events,
            Err(error) => {
                tracing::debug!(%error, "failed to read directory changes");
                return true;
            }
        };
        events.iter().any(|event| match event {
            Event::Lost => true,
            Event::Changed(change) => self.is_watched(&change.path),
        })
    }

    fn is_watched(&self, path: &Path) -> bool {
        self.roots
            .iter()
            .any(|root| path.starts_with(root) || root.starts_with(path))
    }
}

#[cfg(unix)]
fn ready(wake: &EventFd, watches: &Watches, timeout: Option<Duration>) -> io::Result<bool> {
    use std::os::fd::AsFd as _;
    wake.wait_readable(watches.as_fd(), timeout)
}

#[cfg(windows)]
fn ready(wake: &EventFd, watches: &Watches, timeout: Option<Duration>) -> io::Result<bool> {
    use std::os::windows::io::AsHandle as _;
    wake.wait_signaled([watches.as_handle()], timeout)
        .map(|[ready]| ready)
}

fn fresh() -> Result<Watches> {
    Watches::new().map_err(|error| Error::Watch(error.to_string()))
}

fn watch_points(roots: &[PathBuf]) -> Vec<(PathBuf, Reach)> {
    let mut watch_points: Vec<(PathBuf, Reach)> = Vec::new();
    for root in roots {
        for link in symlinks(root) {
            if let Some(parent) = link.parent() {
                watch(&mut watch_points, parent.to_path_buf(), Reach::Entries);
            }
        }
        match root.is_dir() {
            true => watch(&mut watch_points, root.clone(), Reach::Tree),
            false => match root
                .parent()
                .filter(|parent| parent.parent().is_some() && parent.is_dir())
            {
                Some(parent) => watch(&mut watch_points, parent.to_path_buf(), Reach::Contents),
                None => {
                    if let Some(nearest) = nearest(root) {
                        watch(&mut watch_points, nearest, Reach::Entries);
                    }
                }
            },
        }
    }
    watch_points
}

fn watch(watch_points: &mut Vec<(PathBuf, Reach)>, path: PathBuf, reach: Reach) {
    match watch_points
        .iter_mut()
        .find(|(watched, _)| *watched == path)
    {
        Some((_, current_reach)) => *current_reach = (*current_reach).max(reach),
        None => watch_points.push((path, reach)),
    }
}

fn symlinks(path: &Path) -> Vec<PathBuf> {
    let mut walked = PathBuf::new();
    let mut found = Vec::new();
    for component in path.components() {
        walked.push(component);
        let links = std::fs::symlink_metadata(&walked)
            .is_ok_and(|metadata| metadata.file_type().is_symlink());
        if links {
            found.push(walked.clone());
        }
    }
    found
}

fn nearest(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .skip(1)
        .take_while(|above| above.parent().is_some())
        .find(|above| above.is_dir())
        .map(Path::to_path_buf)
}
