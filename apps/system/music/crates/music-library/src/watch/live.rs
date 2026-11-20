use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use rikka_watch::{ChangeKind, Event, Reach, Watcher};
use rikka_worker::{Stop, Worker};

use crate::artwork::IMAGE_EXTENSIONS;
use crate::error::{Error, Result};
use crate::folders;
use crate::paths;
use crate::scan::{RootState, Scanner, Shared, is_audio, outermost};
use crate::watch::mounts::Mounts;

const THREAD: &str = "rikka-music-watch";
const QUIET: Duration = Duration::from_millis(1500);
const LONGEST_WAIT: Duration = Duration::from_secs(10);
const POLL_EVERY: Duration = Duration::from_secs(15 * 60);

enum Signal {
    Path(PathBuf),
    Lost,
    Roots,
    Stop,
}

struct Signals(Sender<Signal>);

impl Stop for Signals {
    fn stop(&self) {
        let _ = self.0.send(Signal::Stop);
    }
}

#[derive(Debug)]
pub(crate) struct Live {
    _mounts: Mounts,
    worker: Worker<Signals>,
}

impl Live {
    pub(crate) fn start(shared: Arc<Shared>, scanner: Arc<Scanner>) -> Result<Live> {
        let (signals, inbox) = mpsc::channel();
        let events = signals.clone();
        let watcher = Watcher::spawn(move |event| forward(&events, event))
            .inspect_err(|error| tracing::warn!(%error, "failed to create music folder watcher"))
            .ok();
        let follower = Follower {
            shared,
            scanner,
            watcher,
            roots: Vec::new(),
            watched: Vec::new(),
            polled: Vec::new(),
            pending: Vec::new(),
            first: None,
            last: None,
            next_poll: Instant::now() + POLL_EVERY,
        };
        let mounted = signals.clone();
        let worker = Worker::spawn(THREAD, Signals(signals), move || {
            Ok::<_, Infallible>(move || follower.follow(&inbox))
        })
        .map_err(|error| Error::Open(format!("{THREAD}: {error}")))?;
        let mounts = Mounts::start(move || {
            let _ = mounted.send(Signal::Roots);
        })?;
        Ok(Live {
            _mounts: mounts,
            worker,
        })
    }

    pub(crate) fn roots_changed(&self) {
        let _ = self.worker.handle().0.send(Signal::Roots);
    }
}

fn forward(signals: &Sender<Signal>, event: Event) {
    let signal = match event {
        Event::Lost => Signal::Lost,
        Event::Changed(change) if change.kind == ChangeKind::Metadata => return,
        Event::Changed(change) => Signal::Path(change.path),
    };
    let _ = signals.send(signal);
}

struct Follower {
    shared: Arc<Shared>,
    scanner: Arc<Scanner>,
    watcher: Option<Watcher>,
    roots: Vec<(PathBuf, RootState)>,
    watched: Vec<PathBuf>,
    polled: Vec<PathBuf>,
    pending: Vec<PathBuf>,
    first: Option<Instant>,
    last: Option<Instant>,
    next_poll: Instant,
}

impl Follower {
    fn follow(mut self, inbox: &Receiver<Signal>) {
        self.reconfigure(false);
        loop {
            let wait = self
                .deadline()
                .map(|deadline| deadline.saturating_duration_since(Instant::now()));
            let signal = match wait {
                Some(wait) => inbox.recv_timeout(wait),
                None => inbox.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            match signal {
                Ok(Signal::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                Ok(Signal::Path(path)) => self.path_changed(path),
                Ok(Signal::Lost) => self.scanner.scan(None, false, None),
                Ok(Signal::Roots) => self.reconfigure(true),
                Err(RecvTimeoutError::Timeout) => self.due(),
            }
        }
    }

    fn deadline(&self) -> Option<Instant> {
        let debounced = self
            .first
            .zip(self.last)
            .map(|(first, last)| (last + QUIET).min(first + LONGEST_WAIT));
        let polled = (!self.polled.is_empty()).then_some(self.next_poll);
        match (debounced, polled) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn due(&mut self) {
        let now = Instant::now();
        if self.first.is_some() && self.deadline().is_some_and(|deadline| deadline <= now) {
            let scopes = std::mem::take(&mut self.pending);
            self.first = None;
            self.last = None;
            if !scopes.is_empty() {
                self.scanner.scan(Some(scopes), false, None);
            }
        }
        if !self.polled.is_empty() && self.next_poll <= now {
            self.next_poll = now + POLL_EVERY;
            self.scanner.scan(Some(self.polled.clone()), false, None);
        }
    }

    fn path_changed(&mut self, path: PathBuf) {
        let Some(root) = self
            .watched
            .iter()
            .find(|root| path.starts_with(root))
            .cloned()
        else {
            return;
        };
        if !self.relevant(&root, &path) {
            return;
        }
        let scope = if path == root {
            root
        } else {
            path.parent().map_or(root, Path::to_path_buf)
        };
        if self
            .pending
            .iter()
            .any(|pending| scope.starts_with(pending))
        {
            return;
        }
        self.pending.retain(|pending| !pending.starts_with(&scope));
        self.pending.push(scope);
        let now = Instant::now();
        self.first.get_or_insert(now);
        self.last = Some(now);
    }

    fn relevant(&self, root: &Path, path: &Path) -> bool {
        if self
            .shared
            .excluded
            .iter()
            .any(|excluded| path.starts_with(excluded))
        {
            return false;
        }
        let hidden = path.strip_prefix(root).map_or(true, |inner| {
            inner
                .components()
                .any(|part| paths::is_hidden(part.as_os_str()))
        });
        if hidden {
            return false;
        }
        match path.extension().and_then(|extension| extension.to_str()) {
            None => true,
            Some(extension) => {
                is_audio(path)
                    || IMAGE_EXTENSIONS
                        .iter()
                        .any(|image| image.eq_ignore_ascii_case(extension))
            }
        }
    }

    fn reconfigure(&mut self, rescan_changed: bool) {
        let configured: Vec<PathBuf> =
            match folders::configured(&self.shared.store, &self.shared.music) {
                Ok(folders) => folders.into_iter().map(|(path, _)| path).collect(),
                Err(error) => {
                    tracing::warn!("failed to list music folders: {error}");
                    return;
                }
            };
        let known = folders::states(&self.shared.store).unwrap_or_default();
        let roots: Vec<(PathBuf, RootState)> = outermost(&configured)
            .into_iter()
            .map(|root| {
                let device = known.get(&root).and_then(|state| state.device);
                let state = RootState::inspect(&root, device);
                (root, state)
            })
            .collect();
        let changed: Vec<PathBuf> = roots
            .iter()
            .filter(|(root, state)| {
                let before = self
                    .roots
                    .iter()
                    .find(|(path, _)| path == root)
                    .map(|(_, state)| state.online)
                    .or_else(|| known.get(root).map(|state| state.online));
                before != Some(state.online)
            })
            .map(|(root, _)| root.clone())
            .collect();
        let eligible: Vec<PathBuf> = roots
            .iter()
            .filter(|(_, state)| state.online && !state.remote)
            .map(|(root, _)| root.clone())
            .collect();
        for root in &self.watched {
            if !eligible.contains(root)
                && let Some(watcher) = self.watcher.as_ref()
            {
                watcher.unwatch(root);
            }
        }
        let mut watched = Vec::new();
        let mut polled: Vec<PathBuf> = roots
            .iter()
            .filter(|(_, state)| state.online && state.remote)
            .map(|(root, _)| root.clone())
            .collect();
        for root in eligible {
            let ok = self.watched.contains(&root)
                || self.watcher.as_ref().is_some_and(|watcher| {
                    watcher
                        .watch(&root, Reach::Tree)
                        .inspect_err(|error| {
                            tracing::warn!("failed to watch {}: {error}", root.display());
                        })
                        .is_ok()
                });
            if ok {
                watched.push(root);
            } else {
                polled.push(root);
            }
        }
        self.watched = watched;
        self.polled = polled;
        self.roots = roots;
        if rescan_changed && !changed.is_empty() {
            self.scanner.scan(Some(changed), false, None);
        }
    }
}
