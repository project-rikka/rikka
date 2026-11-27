use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use rikka_watch::{ChangeKind, Event, Reach, Watcher};
use rikka_worker::{Stop, Worker};

use crate::catalog::{Catalog, Unavailable};
use crate::details::Details;
use crate::entry::{Entry, EntryId};
use crate::error::{Error, Result};
use crate::fit::Fit;
use crate::location::{Location, Locations};
use crate::pool::{Job, Pool, Probe};
use crate::scan::{Found, scan};

const NAME: &str = "rikka-animate-library";
const SETTLE: Duration = Duration::from_millis(250);
const LONGEST_SETTLE: Duration = Duration::from_secs(2);
const PUBLISH_INTERVAL: Duration = Duration::from_millis(200);

pub struct Library {
    commands: Sender<Message>,
    _worker: Worker<Halt>,
}

enum Message {
    Locations(Locations),
    Rescan,
    Watch(Event),
    Probed(Probe),
    Stop,
}

struct Halt(Sender<Message>);

impl Stop for Halt {
    fn stop(&self) {
        if self.0.send(Message::Stop).is_err() {
            tracing::debug!("library worker already stopped");
        }
    }
}

impl Library {
    pub fn open(
        locations: Locations,
        sink: impl Fn(Arc<Catalog>) + Send + Sync + 'static,
    ) -> Result<Library> {
        let (sender, receiver) = mpsc::channel();
        let worker_sender = sender.clone();
        let worker = Worker::spawn(NAME, Halt(sender.clone()), move || {
            let coordinator = Coordinator::new(locations, Box::new(sink), worker_sender)?;
            Ok::<_, Error>(move || coordinator.run(&receiver))
        })?;
        Ok(Library {
            commands: sender,
            _worker: worker,
        })
    }

    pub fn set_locations(&self, locations: Locations) {
        self.send(Message::Locations(locations));
    }

    pub fn rescan(&self) {
        self.send(Message::Rescan);
    }

    fn send(&self, message: Message) {
        if self.commands.send(message).is_err() {
            tracing::warn!("library worker stopped");
        }
    }
}

impl std::fmt::Debug for Library {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Library").finish_non_exhaustive()
    }
}

struct Held {
    origin: usize,
    entry: Arc<Entry>,
}

struct Settle {
    first: Instant,
    last: Instant,
}

struct Coordinator {
    locations: Vec<Location>,
    entries: HashMap<EntryId, Held>,
    unavailable: Vec<Option<Arc<str>>>,
    watcher: Watcher,
    watched: Vec<PathBuf>,
    pool: Pool,
    sink: Box<dyn Fn(Arc<Catalog>) + Send + Sync>,
    stale: BTreeSet<usize>,
    settle: Option<Settle>,
    dirty: bool,
    published: Option<Instant>,
}

impl Coordinator {
    fn new(
        locations: Locations,
        sink: Box<dyn Fn(Arc<Catalog>) + Send + Sync>,
        sender: Sender<Message>,
    ) -> Result<Coordinator> {
        let watch_sender = sender.clone();
        let watcher = Watcher::spawn(move |event| {
            if watch_sender.send(Message::Watch(event)).is_err() {
                tracing::debug!("library worker stopped before a file change");
            }
        })?;
        let pool = Pool::new(move |probe| {
            if sender.send(Message::Probed(probe)).is_err() {
                tracing::debug!("library worker stopped before a probe finished");
            }
        })?;
        let locations: Vec<Location> = locations.iter().cloned().collect();
        Ok(Coordinator {
            unavailable: vec![None; locations.len()],
            locations,
            entries: HashMap::new(),
            watcher,
            watched: Vec::new(),
            pool,
            sink,
            stale: BTreeSet::new(),
            settle: None,
            dirty: false,
            published: None,
        })
    }

    fn run(mut self, receiver: &Receiver<Message>) {
        self.reload();
        loop {
            let message = match self.deadline() {
                Some(deadline) => {
                    receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                }
                None => receiver.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            match message {
                Ok(Message::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                Ok(Message::Locations(locations)) => {
                    self.locations = locations.iter().cloned().collect();
                    self.reload();
                }
                Ok(Message::Rescan) => self.reload(),
                Ok(Message::Watch(event)) => self.changed(&event),
                Ok(Message::Probed(probe)) => self.probed(probe),
                Err(RecvTimeoutError::Timeout) => {}
            }
            self.tick();
        }
    }

    fn reload(&mut self) {
        for path in self.watched.drain(..) {
            self.watcher.unwatch(&path);
        }
        for location in &self.locations {
            match self.watcher.watch(&location.root, Reach::Tree) {
                Ok(()) => self.watched.push(location.root.clone()),
                Err(error) => {
                    tracing::debug!(path = %location.root.display(), %error, "failed to watch library folder");
                }
            }
        }
        self.unavailable = vec![None; self.locations.len()];
        self.stale.clear();
        self.settle = None;
        let previous = self
            .entries
            .drain()
            .map(|(id, held)| (id, held.entry))
            .collect();
        self.fill(&(0..self.locations.len()).collect(), &previous);
        self.published = None;
    }

    fn deadline(&self) -> Option<Instant> {
        let settle = self
            .settle
            .as_ref()
            .map(|settle| (settle.last + SETTLE).min(settle.first + LONGEST_SETTLE));
        let publish = self.dirty.then(|| {
            self.published
                .map_or_else(Instant::now, |at| at + PUBLISH_INTERVAL)
        });
        [settle, publish].into_iter().flatten().min()
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let settled = self.settle.as_ref().is_some_and(|settle| {
            now >= settle.last + SETTLE || now >= settle.first + LONGEST_SETTLE
        });
        if settled {
            self.settle = None;
            let stale = std::mem::take(&mut self.stale);
            self.rescan(&stale);
        }
        let due = self.published.is_none_or(|at| now >= at + PUBLISH_INTERVAL);
        if self.dirty && due {
            self.publish(now);
        }
    }

    fn changed(&mut self, event: &Event) {
        match event {
            Event::Lost => self.stale.extend(0..self.locations.len()),
            Event::Changed(change) => {
                if matches!(change.kind, ChangeKind::Opened | ChangeKind::Closed) {
                    return;
                }
                let touched = self
                    .locations
                    .iter()
                    .enumerate()
                    .filter(|(_, location)| location.contains(&change.path))
                    .map(|(index, _)| index);
                self.stale.extend(touched);
            }
        }
        if self.stale.is_empty() {
            return;
        }
        let now = Instant::now();
        match &mut self.settle {
            Some(settle) => settle.last = now,
            None => {
                self.settle = Some(Settle {
                    first: now,
                    last: now,
                });
            }
        }
    }

    fn rescan(&mut self, indices: &BTreeSet<usize>) {
        let mut previous = HashMap::new();
        self.entries
            .retain(|id, held| match indices.contains(&held.origin) {
                true => {
                    previous.insert(*id, Arc::clone(&held.entry));
                    false
                }
                false => true,
            });
        self.fill(indices, &previous);
    }

    fn fill(&mut self, indices: &BTreeSet<usize>, previous: &HashMap<EntryId, Arc<Entry>>) {
        for &index in indices {
            let Some(location) = self.locations.get(index) else {
                continue;
            };
            match scan(location) {
                Ok(found) => {
                    self.unavailable[index] = None;
                    for found in found {
                        self.admit(index, found, previous);
                    }
                }
                Err(error) => {
                    tracing::debug!(path = %location.root.display(), %error, "failed to scan library");
                    self.unavailable[index] = Some(Arc::from(error.to_string()));
                }
            }
        }
        for id in previous.keys() {
            if !self.entries.contains_key(id) {
                self.pool.forget(*id);
            }
        }
        self.dirty = true;
    }

    fn admit(&mut self, origin: usize, found: Found, previous: &HashMap<EntryId, Arc<Entry>>) {
        let id = EntryId::from_path(&found.path);
        if self.entries.contains_key(&id) {
            return;
        }
        let sidecar = found.sidecar.map(Arc::from);
        let kept = previous.get(&id).filter(|entry| {
            entry.stamp == found.stamp && entry.shelf == found.shelf && entry.format == found.format
        });
        let entry = match kept {
            Some(entry) if entry.folder == found.folder && entry.sidecar == sidecar => {
                Arc::clone(entry)
            }
            Some(entry) => Arc::new(Entry {
                folder: found.folder,
                sidecar,
                ..Entry::clone(entry)
            }),
            None => {
                let path: Arc<std::path::Path> = Arc::from(found.path);
                let name = path
                    .file_stem()
                    .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
                self.pool.want(Job {
                    id,
                    path: Arc::clone(&path),
                    format: found.format,
                    stamp: found.stamp,
                    depth: found
                        .folder
                        .split('/')
                        .filter(|part| !part.is_empty())
                        .count(),
                });
                Arc::new(Entry {
                    id,
                    path,
                    shelf: found.shelf,
                    folder: found.folder,
                    name: Arc::from(name),
                    format: found.format,
                    stamp: found.stamp,
                    sidecar,
                    details: Details::Pending,
                    fit: Fit::judge(found.shelf, found.format, &Details::Pending),
                })
            }
        };
        self.entries.insert(id, Held { origin, entry });
    }

    fn probed(&mut self, probe: Probe) {
        self.pool.unwant(probe.id, probe.stamp);
        let Some(held) = self.entries.get_mut(&probe.id) else {
            return;
        };
        if held.entry.stamp != probe.stamp || !held.entry.details.is_pending() {
            return;
        }
        held.entry = Arc::new(held.entry.with_details(probe.details));
        self.dirty = true;
    }

    fn publish(&mut self, now: Instant) {
        let unavailable = self
            .locations
            .iter()
            .zip(&self.unavailable)
            .filter_map(|(location, reason)| {
                Some(Unavailable {
                    location: location.clone(),
                    reason: Arc::clone(reason.as_ref()?),
                })
            })
            .collect();
        let entries = self.entries.values().map(|held| Arc::clone(&held.entry));
        (self.sink)(Arc::new(Catalog::new(entries, unavailable)));
        self.dirty = false;
        self.published = Some(now);
    }
}
