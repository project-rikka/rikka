mod drafting;
mod effect;
mod observe;
mod operations;
mod report;
pub(crate) mod root;
mod transfer;

use std::collections::{BTreeMap, HashMap};
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use jiff::{Timestamp, Zoned};
use rikka_proton_api::{delay, offload};
use rikka_proton_drive::{DeviceId, DriveClient, EventId, VolumeId};
use rikka_proton_drive_sync_state::{
    Changes, Context, Entry, Operation, Phase, Side, Store, TEMPORARY_PREFIX, reconcile,
};
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time::{Instant, sleep, sleep_until};

use crate::engine::effect::{Invalidation, Outcome};
use crate::engine::observe::next_feed_batch;
use crate::engine::report::DeviceReport;
use crate::engine::root::SyncRoot;
use crate::error::{Error, Result};
use crate::feed::FeedBatch;
use crate::local::files::recycle;
use crate::local::scan::Scanner;
use crate::local::watch::{LocalChanges, LocalWatcher};
use crate::persist::Persist;
use crate::status::{Activity, ItemState, ItemUpdate, Problem, ProblemKind, Status};

const POLL_INTERVAL: Duration = Duration::from_secs(30);
const FEED_FRESHNESS: Duration = Duration::from_secs(60);
const DEBOUNCE_QUIET: Duration = Duration::from_millis(300);
const MAX_DEBOUNCE: Duration = Duration::from_secs(2);
const INITIAL_BACKOFF: Duration = Duration::from_secs(5);
const MAX_BACKOFF: Duration = Duration::from_secs(3600);
const IDLE_WAIT: Duration = Duration::from_secs(3600);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    All,
    Unfed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScanMode {
    Full,
    Dirty,
}

pub(crate) enum Command {
    SyncNow(oneshot::Sender<Result<()>>),
    Shutdown(oneshot::Sender<()>),
}

pub(crate) struct Channels {
    pub(crate) commands: mpsc::Receiver<Command>,
    pub(crate) pause: watch::Receiver<bool>,
    pub(crate) status: watch::Sender<Status>,
    pub(crate) items: broadcast::Sender<ItemUpdate>,
    pub(crate) feed: Option<broadcast::Receiver<FeedBatch>>,
}

#[derive(Default)]
struct Progress {
    done: usize,
    total: usize,
}

pub(crate) struct Engine {
    client: DriveClient,
    persist: Persist,
    roots: Vec<SyncRoot>,
    device: String,
    transfers: usize,
    excluded: Vec<PathBuf>,
    watcher: LocalWatcher,
    channels: Channels,
    cursors: HashMap<VolumeId, EventId>,
    fed_volumes: HashMap<VolumeId, Instant>,
    paused: bool,
    sync_requested: bool,
    refetch: bool,
    shutdown_reply: Option<oneshot::Sender<()>>,
    closed: bool,
    waiters: Vec<oneshot::Sender<Result<()>>>,
    conflicts: Vec<PathBuf>,
    failures: BTreeMap<PathBuf, String>,
    offline: Option<String>,
    progress: Progress,
    previous_operations: Vec<Operation>,
    next_retry: Option<Timestamp>,
    synced_at: Option<Timestamp>,
    report: Option<DeviceReport>,
}

pub(crate) struct Settings {
    pub(crate) device: String,
    pub(crate) device_id: Option<DeviceId>,
    pub(crate) transfers: usize,
    pub(crate) excluded: Vec<PathBuf>,
}

impl Engine {
    pub(crate) fn new(
        client: DriveClient,
        persist: Persist,
        roots: Vec<SyncRoot>,
        settings: Settings,
        watcher: LocalWatcher,
        channels: Channels,
    ) -> Engine {
        Engine {
            report: settings
                .device_id
                .map(|device| DeviceReport::new(client.clone(), device)),
            client,
            persist,
            roots,
            device: settings.device,
            transfers: settings.transfers.max(1),
            excluded: settings.excluded,
            watcher,
            channels,
            cursors: HashMap::new(),
            fed_volumes: HashMap::new(),
            paused: false,
            sync_requested: false,
            refetch: false,
            shutdown_reply: None,
            closed: false,
            waiters: Vec::new(),
            conflicts: Vec::new(),
            failures: BTreeMap::new(),
            offline: None,
            progress: Progress::default(),
            previous_operations: Vec::new(),
            next_retry: None,
            synced_at: None,
        }
    }

    pub(crate) async fn run(mut self) {
        self.publish(Activity::Starting);
        if let Err(error) = self.recover().await {
            tracing::warn!(%error, "failed to recover interrupted operations");
        }
        self.publish(Activity::Scanning);
        self.observe_remote(Scope::All).await;
        self.scan(ScanMode::Full).await;

        let mut poll = Instant::now() + POLL_INTERVAL;
        loop {
            self.drain_commands();
            if self.shutdown_reply.is_some() || self.closed {
                break;
            }
            if self.sync_requested {
                self.sync_requested = false;
                self.publish(Activity::Scanning);
                self.scan(ScanMode::Full).await;
                self.observe_remote(Scope::All).await;
                poll = Instant::now() + POLL_INTERVAL;
            }
            if !self.paused && self.run_pass().await {
                continue;
            }

            self.synced_at = Some(Timestamp::now());
            self.progress = Progress::default();
            let clean = !self.paused
                && self.failures.is_empty()
                && self.offline.is_none()
                && self.next_retry.is_none()
                && self.roots.iter().all(SyncRoot::is_healthy);
            if let Some(report) = &mut self.report {
                report.mark_idle(clean);
                report.send_due().await;
            }
            self.publish(match self.paused {
                true => Activity::Paused,
                false => Activity::Idle,
            });
            for waiter in self.waiters.drain(..) {
                waiter.send(Ok(())).ok();
            }

            let retry = self.next_retry.map_or(Instant::now() + IDLE_WAIT, |at| {
                let wait = Timestamp::now().duration_until(at);
                Instant::now() + Duration::try_from(wait).unwrap_or_default()
            });
            let reporting = self
                .report
                .as_ref()
                .and_then(DeviceReport::next_report)
                .unwrap_or_else(|| Instant::now() + IDLE_WAIT);
            tokio::select! {
                command = self.channels.commands.recv() => match command {
                    Some(command) => self.command(command),
                    None => self.closed = true,
                },
                Ok(()) = self.channels.pause.changed() => {
                    self.paused = *self.channels.pause.borrow_and_update();
                }
                changes = self.watcher.next() => {
                    self.debounce_changes(changes).await;
                    self.scan(ScanMode::Dirty).await;
                }
                feed = next_feed_batch(&mut self.channels.feed) => match feed {
                    Ok(feed) => self.apply_feed_batch(&feed).await,
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => self.channels.feed = None,
                },
                () = sleep_until(poll) => {
                    self.observe_remote(Scope::Unfed).await;
                    poll = Instant::now() + POLL_INTERVAL;
                }
                () = sleep_until(retry) => {}
                () = sleep_until(reporting) => {
                    if let Some(report) = &mut self.report {
                        report.flush().await;
                    }
                }
            }
        }

        for waiter in self.waiters.drain(..) {
            waiter.send(Err(Error::Stopped)).ok();
        }
        if let Some(report) = &mut self.report {
            report.flush().await;
        }
        self.publish(Activity::Stopped);
        if let Some(shutdown_reply) = self.shutdown_reply.take() {
            shutdown_reply.send(()).ok();
        }
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::SyncNow(waiter) => {
                self.sync_requested = true;
                self.waiters.push(waiter);
            }
            Command::Shutdown(done) => self.shutdown_reply = Some(done),
        }
    }

    fn drain_commands(&mut self) {
        if self.channels.pause.has_changed().unwrap_or(false) {
            self.paused = *self.channels.pause.borrow_and_update();
        }
        loop {
            match self.channels.commands.try_recv() {
                Ok(command) => self.command(command),
                Err(mpsc::error::TryRecvError::Empty) => return,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.closed = true;
                    return;
                }
            }
        }
    }

    fn is_interrupted(&mut self) -> bool {
        self.drain_commands();
        self.paused || self.shutdown_reply.is_some() || self.closed
    }

    fn publish(&self, activity: Activity) {
        let mut problems: Vec<Problem> = self.roots.iter().flat_map(SyncRoot::problems).collect();
        problems.extend(
            self.failures
                .iter()
                .map(|(path, reason)| Problem::new(path.clone(), ProblemKind::Transfer, reason)),
        );
        if let Some(reason) = &self.offline {
            problems.push(Problem::new(PathBuf::new(), ProblemKind::Offline, reason));
        }
        let status = Status {
            activity,
            problems,
            conflicts: self.conflicts.clone(),
            online: self.offline.is_none(),
            synced_at: self.synced_at,
        };
        self.channels.status.send_if_modified(|current| {
            let changed = *current != status;
            *current = status;
            changed
        });
    }

    fn notify_item(&self, path: PathBuf, state: ItemState) {
        self.channels.items.send(ItemUpdate::new(path, state)).ok();
    }

    async fn recover(&mut self) -> Result<()> {
        let interrupted_entries = self
            .persist
            .run(|store| store.interrupted_entries())
            .await?;
        for entry in interrupted_entries {
            if let Some(artifact) = &entry.artifact {
                self.recover_entry(&entry, artifact).await?;
            }
            let id = entry.id;
            self.persist.run(move |store| store.release(id)).await?;
        }
        Ok(())
    }

    async fn recover_entry(&mut self, entry: &Entry, artifact: &Path) -> Result<()> {
        let Some(index) = self.roots.iter().position(|root| root.id == entry.root) else {
            return Ok(());
        };
        match &entry.operation {
            Operation::SetAside { local, forget, .. } => {
                let moved = std::fs::symlink_metadata(artifact)
                    .is_ok_and(|metadata| metadata.ino() == *local);
                let root = &self.roots[index];
                let (Some(item), Some(name)) = (
                    root.state.local().get(local).cloned(),
                    artifact.file_name().and_then(|name| name.to_str()),
                ) else {
                    return Ok(());
                };
                if moved {
                    let mut changes = Changes {
                        local: vec![rikka_proton_drive_sync_state::LocalItem {
                            name: name.to_owned(),
                            ..item
                        }],
                        ..Changes::default()
                    };
                    changes.synced_removed.extend(forget.iter().copied());
                    self.commit(index, changes).await?;
                }
            }
            _ => {
                if !artifact
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(TEMPORARY_PREFIX))
                {
                    return Ok(());
                }
                let root = &self.roots[index];
                let displaced = match &entry.operation {
                    Operation::DownloadRevision { synced, .. } => {
                        root.state.synced().get(synced).map(|row| row.local)
                    }
                    _ => None,
                };
                let (artifact, root_path) = (artifact.to_owned(), root.path().to_owned());
                offload(move || discard_leftover(&artifact, &root_path, displaced)).await;
            }
        }
        Ok(())
    }

    async fn commit(&mut self, index: usize, changes: Changes) -> Result<()> {
        if changes.is_empty() {
            return Ok(());
        }
        let id = self.roots[index].id;
        let recorded = changes.clone();
        self.persist
            .run(move |store| store.apply(id, &recorded))
            .await?;
        self.roots[index].state.apply(&changes);
        Ok(())
    }

    async fn debounce_changes(&mut self, first: LocalChanges) {
        self.apply_changes(first);
        let started = Instant::now();
        while started.elapsed() < MAX_DEBOUNCE {
            tokio::select! {
                changes = self.watcher.next() => self.apply_changes(changes),
                () = sleep(DEBOUNCE_QUIET) => return,
            }
        }
    }

    fn apply_changes(&mut self, changes: LocalChanges) {
        if changes.rescan {
            for root in &mut self.roots {
                root.rescan = true;
            }
        }
        for path in changes.paths {
            self.mark_changed(path);
        }
    }

    fn mark_changed(&mut self, path: PathBuf) {
        let is_excluded = self
            .excluded
            .iter()
            .any(|excluded| path.starts_with(excluded));
        let is_temporary = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(TEMPORARY_PREFIX));
        if is_excluded || is_temporary {
            return;
        }
        let Some(root) = self
            .roots
            .iter_mut()
            .find(|root| path.starts_with(&root.record.local_path))
        else {
            return;
        };
        if path == root.record.local_path {
            root.dirty.insert(path);
            return;
        }
        if let Some(parent) = path.parent() {
            root.dirty.insert(parent.to_owned());
        }
        if std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_dir()) {
            root.dirty.insert(path);
        }
    }

    async fn scan(&mut self, mode: ScanMode) {
        while let Some(changes) = self.watcher.try_next() {
            self.apply_changes(changes);
        }
        for index in 0..self.roots.len() {
            if let Err(error) = self.scan_root(index, mode).await {
                tracing::warn!(root = %self.roots[index].path().display(), %error, "failed to scan folder");
            }
        }
    }

    async fn scan_root(&mut self, index: usize, mode: ScanMode) -> Result<()> {
        let path = self.roots[index].path().to_owned();
        let metadata = std::fs::symlink_metadata(&path)
            .ok()
            .filter(|current| current.is_dir());
        let Some(metadata) = metadata else {
            self.roots[index].local_missing = true;
            return Ok(());
        };
        let mut full = mode == ScanMode::Full || self.roots[index].rescan;
        if self.roots[index].local_missing {
            self.roots[index].local_missing = false;
            self.watcher.rewatch(&path);
            full = true;
        }
        if metadata.ino() != self.roots[index].record.local_root {
            self.replace_local_root(index, metadata.ino()).await?;
            full = true;
        }

        let root = &mut self.roots[index];
        let dirty = std::mem::take(&mut root.dirty);
        root.rescan = false;
        if !full && dirty.is_empty() {
            return Ok(());
        }
        let known = root.state.local().clone();
        let root_inode = root.state.local_root();
        let excluded = self.excluded.clone();
        let scanned = offload(move || {
            let scanner = Scanner::new(&path, root_inode, &excluded, &known);
            match full {
                true => scanner.full(),
                false => scanner.folders(&dirty),
            }
        })
        .await
        .map_err(Error::io(self.roots[index].path()))?;

        let root = &mut self.roots[index];
        if full {
            root.invalid.clear();
            root.unreadable.clear();
        }
        root.invalid.extend(scanned.invalid);
        root.unreadable.extend(scanned.unreadable);
        self.commit(index, scanned.changes).await
    }

    async fn replace_local_root(&mut self, index: usize, inode: u64) -> Result<()> {
        let root = &mut self.roots[index];
        tracing::warn!(root = %root.path().display(), "sync root replaced, rematching");
        root.record.local_root = inode;
        let (id, record) = (root.id, root.record.clone());
        let state = self
            .persist
            .run(move |store| {
                store.clear(id, Side::Local)?;
                store.update_root(id, &record)?;
                store.load(id, &record)
            })
            .await?;
        self.roots[index].state = state;
        Ok(())
    }

    async fn run_pass(&mut self) -> bool {
        self.scan(ScanMode::Dirty).await;
        if std::mem::take(&mut self.refetch) {
            self.observe_remote(Scope::All).await;
        }
        let context = Context::new(self.device.clone(), Zoned::now().date());
        let now = Timestamp::now();
        let mut work: Vec<(usize, Entry)> = Vec::new();
        self.next_retry = None;
        self.failures.clear();
        for index in 0..self.roots.len() {
            let root = &self.roots[index];
            if !root.is_healthy() {
                continue;
            }
            let plan = reconcile(&root.state, &context).into_operations();
            let id = root.id;
            let entries = match self
                .persist
                .run(move |store: &mut Store| store.schedule(id, &plan))
                .await
            {
                Ok(entries) => entries,
                Err(error) => {
                    tracing::warn!(%error, "failed to journal plan");
                    continue;
                }
            };
            for entry in entries {
                if let Some(error) = &entry.error {
                    self.failures.insert(
                        self.roots[index].operation_path(&entry.operation),
                        error.clone(),
                    );
                }
                match entry.is_due(now) {
                    true => work.push((index, entry)),
                    false => {
                        self.next_retry = entry.retry_at.into_iter().chain(self.next_retry).min();
                    }
                }
            }
        }
        if work.is_empty() {
            self.previous_operations.clear();
            return false;
        }

        let operations: Vec<Operation> = work
            .iter()
            .map(|(_, entry)| entry.operation.clone())
            .collect();
        if operations == self.previous_operations {
            for (index, entry) in work {
                self.record_failure(index, entry, "operation made no progress".to_owned())
                    .await;
            }
            self.previous_operations.clear();
            return false;
        }
        self.previous_operations = operations;
        if let Some(report) = &mut self.report {
            report.mark_dirty();
        }
        self.progress.total = self.progress.done + work.len();
        self.publish(self.syncing());

        let (transfers, others): (Vec<_>, Vec<_>) = work
            .into_iter()
            .partition(|(_, entry)| entry.operation.phase() == Phase::Transfer);
        for phase in [Phase::Bookkeeping, Phase::Move, Phase::Create, Phase::Trash] {
            for (index, entry) in others
                .iter()
                .filter(|(_, entry)| entry.operation.phase() == phase)
            {
                if self.is_interrupted() {
                    return true;
                }
                self.execute(*index, entry.clone()).await;
            }
        }
        if !self.is_interrupted() {
            self.transfer(transfers).await;
        }
        true
    }

    fn syncing(&self) -> Activity {
        Activity::Syncing {
            done: self.progress.done,
            total: self.progress.total,
        }
    }

    async fn execute(&mut self, index: usize, entry: Entry) {
        let id = entry.id;
        if let Err(error) = self.persist.run(move |store| store.begin(id, None)).await {
            tracing::warn!(%error, "failed to journal operation start");
            return;
        }
        let path = self.roots[index].operation_path(&entry.operation);
        if let Some(direction) = direction_of(&entry.operation) {
            self.notify_item(path.clone(), ItemState::Syncing(direction));
        }
        let outcome = self.perform(index, &entry).await;
        self.complete_operation(index, entry, path, outcome).await;
    }

    async fn complete_operation(
        &mut self,
        index: usize,
        entry: Entry,
        path: PathBuf,
        outcome: Result<Outcome>,
    ) {
        self.progress.done += 1;
        match outcome {
            Ok(Outcome::Done(effect)) => {
                let (root, id) = (self.roots[index].id, entry.id);
                let recorded = effect.changes.clone();
                match self
                    .persist
                    .run(move |store| store.finish(root, id, &recorded))
                    .await
                {
                    Ok(()) => self.roots[index].state.apply(&effect.changes),
                    Err(error) => {
                        tracing::warn!(%error, "failed to record completed operation");
                        return;
                    }
                }
                if let Some(copy) = &effect.conflict {
                    self.conflicts.push(copy.clone());
                }
                if direction_of(&entry.operation).is_some() || effect.conflict.is_some() {
                    self.notify_item(effect.path.unwrap_or(path), effect.state);
                }
            }
            Ok(Outcome::Skipped {
                reason,
                invalidation,
            }) => {
                tracing::debug!(path = %path.display(), %reason, "skipped operation");
                match invalidation {
                    Invalidation::Nothing => {}
                    Invalidation::Local(folder) => {
                        self.roots[index].dirty.insert(folder);
                    }
                    Invalidation::Remote => self.refetch = true,
                }
                let id = entry.id;
                if let Err(error) = self.persist.run(move |store| store.release(id)).await {
                    tracing::warn!(%error, "failed to release journaled operation");
                }
            }
            Err(error) => {
                tracing::debug!(path = %path.display(), %error, "operation failed");
                self.notify_item(path.clone(), ItemState::Failed(error.to_string()));
                self.failures.insert(path, error.to_string());
                self.record_failure(index, entry, error.to_string()).await;
            }
        }
        self.publish(self.syncing());
    }

    async fn record_failure(&mut self, index: usize, entry: Entry, reason: String) {
        let wait = delay::doubling(INITIAL_BACKOFF, entry.attempts, MAX_BACKOFF);
        let retry_at = Timestamp::now()
            .checked_add(wait)
            .unwrap_or_else(|_| Timestamp::now());
        self.next_retry = Some(
            self.next_retry
                .map_or(retry_at, |current| current.min(retry_at)),
        );
        let id = entry.id;
        if let Err(error) = self
            .persist
            .run(move |store| store.fail(id, &reason, retry_at).map(drop))
            .await
        {
            tracing::warn!(root = %self.roots[index].path().display(), %error, "failed to journal operation failure");
        }
    }
}

fn discard_leftover(artifact: &Path, root: &Path, displaced: Option<u64>) {
    let original = displaced.is_some_and(|local| {
        std::fs::symlink_metadata(artifact).is_ok_and(|metadata| metadata.ino() == local)
    });
    if !original {
        if let Err(error) = std::fs::remove_file(artifact)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(path = %artifact.display(), %error, "failed to remove interrupted download");
        }
        return;
    }
    if let Err(error) = recycle(artifact, root) {
        tracing::warn!(path = %artifact.display(), %error, "failed to remove file displaced by interrupted download");
    }
}

pub(crate) fn direction_of(operation: &Operation) -> Option<crate::status::Direction> {
    match operation.direction() {
        rikka_proton_drive_sync_state::Direction::Up => Some(crate::status::Direction::Up),
        rikka_proton_drive_sync_state::Direction::Down => Some(crate::status::Direction::Down),
        rikka_proton_drive_sync_state::Direction::Both => None,
    }
}
