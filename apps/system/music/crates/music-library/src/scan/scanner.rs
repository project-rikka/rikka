use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::SyncSender;
use std::time::{Duration, Instant};

use rikka_worker::{Broadcast, CommandReceiver, CommandSender, Observable, Worker};

use crate::artwork::{ArtworkCache, FolderImages};
use crate::error::{Error, Result};
use crate::folders;
use crate::ids::TrackId;
use crate::index::{self, Changes, Stat};
use crate::model::{Change, Progress, Summary};
use crate::paths;
use crate::scan::halt::Halt;
use crate::scan::pool;
use crate::scan::roots::{RootState, outermost};
use crate::scan::walk::Walk;
use crate::store::Store;
use crate::time;

const THREAD: &str = "rikka-music-scan";
const BATCH: usize = 128;
const REGROUP_EVERY: Duration = Duration::from_secs(5);
const IDENTITY_MEMORY_MS: i64 = 30 * 24 * 60 * 60 * 1000;

#[derive(Debug)]
pub(crate) struct Shared {
    pub(crate) store: Arc<Store>,
    pub(crate) artwork: ArtworkCache,
    pub(crate) progress: Observable<Progress>,
    pub(crate) listeners: Broadcast<Change>,
    pub(crate) music: PathBuf,
    pub(crate) excluded: Vec<PathBuf>,
    pub(crate) epoch: AtomicU64,
}

pub(crate) type Reply = SyncSender<Result<Summary>>;

enum Request {
    Scan {
        scopes: Option<Vec<PathBuf>>,
        force: bool,
        epoch: u64,
        reply: Option<Reply>,
    },
    Forget {
        root: PathBuf,
        reply: Option<Reply>,
    },
    Stop,
}

#[derive(Debug)]
pub(crate) struct Scanner {
    shared: Arc<Shared>,
    worker: Worker<CommandSender<Request>>,
}

impl Scanner {
    pub(crate) fn start(shared: Arc<Shared>) -> Result<Scanner> {
        let (sender, commands) = rikka_worker::command_channel(|| Request::Stop)
            .map_err(|error| Error::Open(error.to_string()))?;
        let running = Arc::clone(&shared);
        let worker = Worker::spawn(THREAD, sender, move || {
            Ok::<_, Infallible>(move || Run::new(running).serve(&commands))
        })
        .map_err(|error| Error::Open(error.to_string()))?;
        Ok(Scanner { shared, worker })
    }

    pub(crate) fn scan(&self, scopes: Option<Vec<PathBuf>>, force: bool, reply: Option<Reply>) {
        let epoch = self.shared.epoch.load(Ordering::SeqCst);
        self.send(Request::Scan {
            scopes,
            force,
            epoch,
            reply,
        });
    }

    pub(crate) fn forget(&self, root: PathBuf, reply: Option<Reply>) {
        self.send(Request::Forget { root, reply });
    }

    pub(crate) fn cancel(&self) {
        self.shared.epoch.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn stop(&self) {
        self.cancel();
        self.worker.stop();
    }

    fn send(&self, request: Request) {
        if let Err(error) = self.worker.handle().send(request) {
            tracing::debug!("failed to send music library scan request: {error}");
        }
    }
}

fn answer(reply: Option<Reply>, outcome: Result<Summary>) {
    if let Some(reply) = reply {
        let _ = reply.send(outcome);
    }
}

struct Run {
    shared: Arc<Shared>,
    images: FolderImages,
    grouped_once: bool,
}

struct Target {
    root: PathBuf,
    under: PathBuf,
}

impl Run {
    fn new(shared: Arc<Shared>) -> Run {
        Run {
            shared,
            images: FolderImages::default(),
            grouped_once: false,
        }
    }

    fn serve(mut self, commands: &CommandReceiver<Request>) {
        loop {
            if let Err(error) = commands.wait() {
                tracing::error!("music library scanner reply dropped: {error}");
                return;
            }
            let mut scopes: Option<Vec<PathBuf>> = Some(Vec::new());
            let mut force = false;
            let mut replies = Vec::new();
            let mut scan_requested = false;
            for request in commands.drain() {
                match request {
                    Request::Stop => return,
                    Request::Forget { root, reply } => {
                        let outcome = self.forget(&root);
                        let failed = outcome.is_err();
                        answer(reply, outcome.map(|()| Summary::default()));
                        if !failed {
                            scopes = None;
                            scan_requested = true;
                        }
                    }
                    Request::Scan { epoch, reply, .. }
                        if epoch < self.shared.epoch.load(Ordering::SeqCst) =>
                    {
                        answer(reply, Err(Error::Cancelled));
                    }
                    Request::Scan {
                        scopes: requested,
                        force: forced,
                        reply,
                        ..
                    } => {
                        scan_requested = true;
                        force |= forced;
                        replies.extend(reply);
                        scopes = match (scopes, requested) {
                            (Some(mut queued), Some(requested)) => {
                                queued.extend(requested);
                                Some(queued)
                            }
                            _ => None,
                        };
                    }
                }
            }
            if !scan_requested {
                continue;
            }
            let epoch = self.shared.epoch.load(Ordering::SeqCst);
            let outcome = self.scan(scopes, force, epoch);
            self.shared.progress.update(Progress::default());
            if let Err(error) = &outcome {
                tracing::warn!("music library scan stopped: {error}");
            }
            for reply in replies {
                let _ = reply.send(outcome.clone());
            }
        }
    }

    fn forget(&mut self, root: &Path) -> Result<()> {
        let root = root.to_path_buf();
        let now = time::now();
        self.shared
            .store
            .write(move |transaction| index::forget_root(transaction, &root, now))?;
        index::aggregate(
            &self.shared.store,
            &self.shared.artwork,
            &mut self.images,
            true,
        )?;
        self.shared.listeners.send(&Change::Indexed);
        Ok(())
    }

    fn scan(&mut self, scopes: Option<Vec<PathBuf>>, force: bool, epoch: u64) -> Result<Summary> {
        let shared = Arc::clone(&self.shared);
        let stop = Halt::new(&shared.epoch, epoch);
        shared.progress.update(Progress {
            scanning: true,
            ..Progress::default()
        });
        let online = self.refresh_roots()?;
        let targets = targets(&online, scopes);
        let mut walk = Walk::excluding(&shared.excluded);
        let mut stats_by_walk: Vec<(usize, Vec<Stat>)> = Vec::new();
        for (index, target) in targets.iter().enumerate() {
            walk.walk(&target.under, stop);
            stats_by_walk.push((index, std::mem::take(&mut walk.found)));
            let total = stats_by_walk
                .iter()
                .map(|(_, stats)| stats.len() as u64)
                .sum();
            shared.progress.update(Progress {
                scanning: true,
                found: total,
                ..Progress::default()
            });
        }
        if stop.is_set() {
            return Err(Error::Cancelled);
        }
        let mut summary = Summary::default();
        let mut reading: Vec<(usize, Option<TrackId>, Stat)> = Vec::new();
        let now = time::now();
        for (index, stats) in stats_by_walk {
            let target = &targets[index];
            let known = known_under(&shared.store, &target.under)?;
            let mut changes = Changes::new(target.root.clone());
            let present: HashSet<&Path> = stats.iter().map(|stat| stat.path.as_path()).collect();
            for (path, row) in &known.tracks {
                if !present.contains(path.as_path()) {
                    changes.gone.push(row.0);
                }
            }
            for path in known.failures.keys() {
                if !present.contains(path.as_path()) {
                    changes.recovered.push(path.clone());
                }
            }
            for stat in stats {
                match known.tracks.get(&stat.path) {
                    Some((_, indexed)) if !force && indexed.same_stamp(&stat) => {
                        summary.unchanged += 1
                    }
                    Some((id, _)) => reading.push((index, Some(*id), stat)),
                    None => match known.failures.get(&stat.path) {
                        Some(failed) if !force && failed.same_stamp(&stat) => {
                            summary.unchanged += 1;
                        }
                        _ => reading.push((index, None, stat)),
                    },
                }
            }
            if !changes.is_empty() {
                let applied = shared
                    .store
                    .write(move |transaction| index::apply(transaction, &changes, now))?;
                add(&mut summary, applied);
            }
        }
        let to_read = reading.len() as u64;
        let found_total = summary.unchanged + to_read;
        shared.progress.update(Progress {
            scanning: true,
            found: found_total,
            read: 0,
            to_read,
        });
        let mut pending: HashMap<usize, Changes> = HashMap::new();
        let mut read = 0u64;
        let mut regrouped = Instant::now();
        let mut failure: Option<Error> = None;
        pool::read_all(reading, &shared.artwork, stop, |index, id, outcome| {
            if failure.is_some() {
                return;
            }
            let changes = pending
                .entry(index)
                .or_insert_with(|| Changes::new(targets[index].root.clone()));
            match (id, outcome) {
                (_, pool::Outcome::Failed(stat)) => changes.failed.push(stat),
                (Some(id), pool::Outcome::Indexed(record)) => changes.updated.push((id, *record)),
                (None, pool::Outcome::Indexed(record)) => changes.added.push(*record),
            }
            read += 1;
            let full = changes.updated.len() + changes.added.len() + changes.failed.len() >= BATCH;
            if full {
                let changes = pending.remove(&index).unwrap_or_default();
                match shared
                    .store
                    .write(move |transaction| index::apply(transaction, &changes, time::now()))
                {
                    Ok(applied) => add(&mut summary, applied),
                    Err(error) => failure = Some(error),
                }
                shared.progress.update(Progress {
                    scanning: true,
                    found: found_total,
                    read,
                    to_read,
                });
                if regrouped.elapsed() >= REGROUP_EVERY {
                    regrouped = Instant::now();
                    match index::aggregate(&shared.store, &shared.artwork, &mut self.images, false)
                    {
                        Ok(()) => shared.listeners.send(&Change::Indexed),
                        Err(error) => failure = Some(error),
                    }
                }
            }
        });
        if let Some(error) = failure {
            return Err(error);
        }
        for (_, changes) in pending {
            if changes.is_empty() {
                continue;
            }
            let applied = shared
                .store
                .write(move |transaction| index::apply(transaction, &changes, time::now()))?;
            add(&mut summary, applied);
        }
        let complete = !stop.is_set();
        if complete {
            let whole: Vec<PathBuf> = targets
                .iter()
                .filter(|target| target.root == target.under)
                .map(|target| target.root.clone())
                .collect();
            shared.store.write(move |transaction| {
                for root in &whole {
                    index::retire_unseen(transaction, root, now)?;
                }
                index::prune_identities(transaction, now - IDENTITY_MEMORY_MS)
            })?;
        }
        let changed = summary.added + summary.updated + summary.moved + summary.removed > 0;
        if changed || force || !self.grouped_once {
            index::aggregate(&shared.store, &shared.artwork, &mut self.images, complete)?;
            self.grouped_once = true;
            shared.listeners.send(&Change::Indexed);
        }
        if complete {
            Ok(summary)
        } else {
            Err(Error::Cancelled)
        }
    }

    fn refresh_roots(&self) -> Result<Vec<(PathBuf, RootState)>> {
        let shared = &self.shared;
        let configured: Vec<PathBuf> = folders::configured(&shared.store, &shared.music)?
            .into_iter()
            .map(|(path, _)| path)
            .collect();
        let known = folders::states(&shared.store)?;
        let states: Vec<(PathBuf, RootState)> = outermost(&configured)
            .into_iter()
            .map(|root| {
                let state =
                    RootState::inspect(&root, known.get(&root).and_then(|known| known.device));
                (root, state)
            })
            .collect();
        let moved = states.len() != known.len()
            || states.iter().any(|(root, state)| {
                known.get(root).is_none_or(|stored_state| {
                    stored_state.online != state.online || stored_state.remote != state.remote
                })
            });
        let unchanged = !moved
            && states
                .iter()
                .all(|(root, state)| known.get(root) == Some(state));
        if !unchanged {
            let saved = states.clone();
            shared
                .store
                .write(move |transaction| folders::save_states(transaction, &saved))?;
        }
        if moved {
            shared.listeners.send(&Change::Folders);
        }
        Ok(states
            .into_iter()
            .filter(|(_, state)| state.online)
            .collect())
    }
}

fn targets(online: &[(PathBuf, RootState)], scopes: Option<Vec<PathBuf>>) -> Vec<Target> {
    let Some(scopes) = scopes else {
        return online
            .iter()
            .map(|(root, _)| Target {
                root: root.clone(),
                under: root.clone(),
            })
            .collect();
    };
    let mut distinct: Vec<PathBuf> = Vec::new();
    for scope in scopes {
        if distinct.iter().any(|other| scope.starts_with(other)) {
            continue;
        }
        distinct.retain(|other| !other.starts_with(&scope));
        distinct.push(scope);
    }
    distinct
        .into_iter()
        .filter_map(|under| {
            let (root, _) = online.iter().find(|(root, _)| under.starts_with(root))?;
            Some(Target {
                root: root.clone(),
                under,
            })
        })
        .collect()
}

fn add(summary: &mut Summary, applied: Summary) {
    summary.added += applied.added;
    summary.updated += applied.updated;
    summary.moved += applied.moved;
    summary.removed += applied.removed;
}

struct Known {
    tracks: HashMap<PathBuf, (TrackId, Stat)>,
    failures: HashMap<PathBuf, Stat>,
}

fn known_under(store: &Store, under: &Path) -> Result<Known> {
    let (low, high) = paths::beneath(under);
    store.read(|transaction| {
        let tracks = transaction
            .prepare_cached(
                "SELECT id, path, device, inode, size, modified FROM tracks
                 WHERE path >= ?1 AND path < ?2",
            )?
            .query_map([&low, &high], |row| {
                let path = paths::from_bytes(row.get(1)?);
                Ok((
                    path.clone(),
                    (
                        TrackId::new(row.get(0)?),
                        Stat {
                            path,
                            device: row.get(2)?,
                            inode: row.get(3)?,
                            size: row.get(4)?,
                            modified: row.get(5)?,
                        },
                    ),
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let failures = transaction
            .prepare_cached(
                "SELECT path, size, modified FROM failures WHERE path >= ?1 AND path < ?2",
            )?
            .query_map([&low, &high], |row| {
                let path = paths::from_bytes(row.get(0)?);
                Ok((
                    path.clone(),
                    Stat {
                        path,
                        device: 0,
                        inode: 0,
                        size: row.get(1)?,
                        modified: row.get(2)?,
                    },
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Known { tracks, failures })
    })
}
