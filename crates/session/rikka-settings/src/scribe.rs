use rikka_sync::{Condvar, Mutex, MutexGuard};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Once, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use rikka_fs::Replace;

use crate::atomic;

const SETTLE: Duration = Duration::from_millis(120);

const PATIENCE: Duration = Duration::from_millis(750);

static SCRIBE: OnceLock<Scribe> = OnceLock::new();

pub(crate) fn scribe() -> &'static Scribe {
    let scribe = SCRIBE.get_or_init(Scribe::default);
    scribe.start();
    scribe
}

pub fn flush_pending_writes() {
    if let Some(scribe) = SCRIBE.get() {
        scribe.settle(None);
    }
}

pub(crate) fn wait_for_writes(path: &Path) {
    if let Some(scribe) = SCRIBE.get() {
        scribe.settle(Some(path));
    }
}

pub(crate) fn failed(path: &Path) -> bool {
    SCRIBE.get().is_some_and(|scribe| scribe.failed(path))
}

#[derive(Debug)]
pub(crate) struct Scribe {
    desk: Mutex<Desk>,
    knock: Condvar,
    flushed: Condvar,
    start: Once,
}

#[derive(Debug, Default)]
struct Desk {
    pending: BTreeMap<PathBuf, Option<String>>,
    writing: BTreeSet<PathBuf>,
    failed: BTreeMap<PathBuf, io::Error>,
    revision: u64,
    urgent: bool,
    keeping: bool,
}

impl Scribe {
    fn start(&'static self) {
        self.start.call_once(|| {
            self.desk().keeping = true;
            if let Err(error) = thread::Builder::new()
                .name("rikka-settings".to_owned())
                .spawn(move || self.keep())
            {
                self.desk().keeping = false;
                tracing::error!("failed to spawn settings writer, writing synchronously: {error}");
            }
        });
    }

    pub(crate) fn write(&self, path: PathBuf, document: Option<String>) -> Option<io::Error> {
        let mut desk = self.desk();
        let failure = desk.failed.remove(&path);
        if !desk.keeping {
            drop(desk);
            self.put(&path, document.as_deref());
            return failure;
        }
        desk.revision += 1;
        desk.pending.insert(path, document);
        self.knock.notify_one();
        failure
    }

    fn failed(&self, path: &Path) -> bool {
        self.desk().failed.contains_key(path)
    }

    fn settle(&self, path: Option<&Path>) {
        let mut desk = self.desk();
        if !desk.owes(path) {
            return;
        }
        desk.urgent = true;
        self.knock.notify_one();
        while desk.owes(path) {
            desk = self.flushed.wait(desk);
        }
    }

    fn keep(&self) {
        loop {
            let batch = {
                let mut desk = self.desk();
                while desk.pending.is_empty() {
                    desk = self.knock.wait(desk);
                }
                desk = self.linger(desk);
                let batch = std::mem::take(&mut desk.pending);
                desk.writing = batch.keys().cloned().collect();
                batch
            };
            for (path, document) in batch {
                self.put(&path, document.as_deref());
                self.desk().writing.remove(&path);
                self.flushed.notify_all();
            }
        }
    }

    fn linger<'a>(&'a self, mut desk: MutexGuard<'a, Desk>) -> MutexGuard<'a, Desk> {
        let until = Instant::now() + PATIENCE;
        loop {
            if desk.urgent {
                break;
            }
            let Some(left) = until.checked_duration_since(Instant::now()) else {
                break;
            };
            let revision = desk.revision;
            desk = self.knock.wait_timeout(desk, SETTLE.min(left)).0;
            if desk.revision == revision {
                break;
            }
        }
        desk.urgent = false;
        desk
    }

    fn put(&self, path: &Path, document: Option<&str>) {
        let encoded = match document {
            Some(text) => Replace::new(path).private().write(text),
            None => atomic::remove(path),
        };
        let mut desk = self.desk();
        match encoded {
            Ok(()) => {
                desk.failed.remove(path);
            }
            Err(error) => {
                tracing::error!("failed to write `{}`: {error}", path.display());
                desk.failed.insert(path.to_path_buf(), error);
            }
        }
    }

    fn desk(&self) -> MutexGuard<'_, Desk> {
        self.desk.lock()
    }
}

impl Default for Scribe {
    fn default() -> Self {
        Self {
            desk: Mutex::default(),
            knock: Condvar::new(),
            flushed: Condvar::new(),
            start: Once::new(),
        }
    }
}

impl Desk {
    fn owes(&self, path: Option<&Path>) -> bool {
        match path {
            Some(path) => self.pending.contains_key(path) || self.writing.contains(path),
            None => !self.pending.is_empty() || !self.writing.is_empty(),
        }
    }
}
