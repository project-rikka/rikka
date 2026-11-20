use rikka_sync::Mutex;
use std::collections::HashSet;
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_watch::{Event, Reach, Watcher};
use tokio::sync::Notify;

use crate::error::{Error, Result};

const MAX_PENDING_PATHS: usize = 4096;

#[derive(Debug, Default)]
pub(crate) struct LocalChanges {
    pub(crate) paths: HashSet<PathBuf>,
    pub(crate) rescan: bool,
}

impl LocalChanges {
    fn is_empty(&self) -> bool {
        self.paths.is_empty() && !self.rescan
    }

    fn record(&mut self, path: PathBuf) {
        if self.rescan {
            return;
        }
        if self.paths.len() >= MAX_PENDING_PATHS {
            self.paths.clear();
            self.rescan = true;
            return;
        }
        self.paths.insert(path);
    }

    fn request_rescan(&mut self) {
        self.paths.clear();
        self.rescan = true;
    }
}

#[derive(Default)]
struct PendingChanges {
    changes: Mutex<LocalChanges>,
    ready: Notify,
}

impl PendingChanges {
    fn update(&self, apply: impl FnOnce(&mut LocalChanges)) {
        apply(&mut self.changes.lock());
        self.ready.notify_one();
    }

    fn take(&self) -> Option<LocalChanges> {
        let mut changes = self.changes.lock();
        (!changes.is_empty()).then(|| mem::take(&mut *changes))
    }
}

pub(crate) struct LocalWatcher {
    watcher: Watcher,
    pending: Arc<PendingChanges>,
}

impl LocalWatcher {
    pub(crate) fn start(roots: &[PathBuf]) -> Result<LocalWatcher> {
        let pending = Arc::new(PendingChanges::default());
        let sink = Arc::clone(&pending);
        let watcher = Watcher::spawn(move |event| record_event(&sink, event))
            .map_err(|error| Error::Watch(PathBuf::new(), error.to_string()))?;
        for root in roots {
            watcher
                .watch(root, Reach::Tree)
                .map_err(|error| Error::Watch(root.clone(), error.to_string()))?;
        }
        Ok(LocalWatcher { watcher, pending })
    }

    pub(crate) async fn next(&mut self) -> LocalChanges {
        loop {
            if let Some(changes) = self.pending.take() {
                return changes;
            }
            self.pending.ready.notified().await;
        }
    }

    pub(crate) fn try_next(&mut self) -> Option<LocalChanges> {
        self.pending.take()
    }

    pub(crate) fn rewatch(&mut self, root: &Path) {
        self.watcher.unwatch(root);
        if let Err(error) = self.watcher.watch(root, Reach::Tree) {
            tracing::warn!(root = %root.display(), %error, "failed to rewatch folder");
        }
    }
}

fn record_event(pending: &PendingChanges, event: Event) {
    match event {
        Event::Lost => pending.update(LocalChanges::request_rescan),
        Event::Changed(change) => pending.update(|changes| changes.record(change.path)),
    }
}
