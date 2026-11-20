use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use music_core::{Error, Result, Service};
use rikka_watch::Reach;
use rikka_worker::{Broadcast, Stop, Subscription, Worker};

use crate::blend::{Blend, BlendId};
use crate::folder;
use crate::ledger::Ledger;

const NAME: &str = "rikka-music-playlists";
const QUIET: Duration = Duration::from_millis(300);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outside {
    Edited(Arc<Blend>),
    Gone(BlendId),
}

enum Signal {
    Changed,
    Stopped,
}

struct Halt(Sender<Signal>);

impl Stop for Halt {
    fn stop(&self) {
        let _ = self.0.send(Signal::Stopped);
    }
}

pub(crate) struct Watcher {
    active: Option<Active>,
    listeners: Arc<Broadcast<Outside>>,
}

struct Active {
    _worker: Worker<Halt>,
    _watch: rikka_watch::Watcher,
}

impl Watcher {
    pub(crate) fn start(
        folder: PathBuf,
        services: Vec<Service>,
        ledger: Ledger,
    ) -> Result<Watcher> {
        let listeners = Arc::new(Broadcast::new());
        let (signals, received) = mpsc::channel();
        let changes = signals.clone();
        let watch = rikka_watch::Watcher::spawn(move |_| {
            let _ = changes.send(Signal::Changed);
        })
        .and_then(|watch| {
            watch.watch(&folder, Reach::Contents)?;
            Ok(watch)
        });
        let watch = match watch {
            Ok(watch) => watch,
            Err(error) => {
                tracing::warn!(
                    "failed to watch {} for playlists: {error}",
                    folder.display()
                );
                return Ok(Watcher {
                    active: None,
                    listeners,
                });
            }
        };
        let outside = Arc::clone(&listeners);
        let worker = Worker::spawn(NAME, Halt(signals), move || {
            Ok::<_, Infallible>(move || watching(&folder, &services, &ledger, &received, &outside))
        })
        .map_err(|error| Error::Spawn(format!("{NAME}: {error}")))?;
        Ok(Watcher {
            active: Some(Active {
                _worker: worker,
                _watch: watch,
            }),
            listeners,
        })
    }

    pub(crate) fn outside(&self) -> Subscription<Outside> {
        self.listeners.subscribe()
    }
}

impl std::fmt::Debug for Watcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Watcher")
            .field("watching", &self.active.is_some())
            .finish_non_exhaustive()
    }
}

fn watching(
    folder: &Path,
    services: &[Service],
    ledger: &Ledger,
    signals: &Receiver<Signal>,
    listeners: &Broadcast<Outside>,
) {
    while let Ok(Signal::Changed) = signals.recv() {
        loop {
            match signals.recv_timeout(QUIET) {
                Ok(Signal::Changed) => continue,
                Ok(Signal::Stopped) | Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => break,
            }
        }
        for change in compared(folder, services, ledger) {
            listeners.send(&change);
        }
    }
}

fn compared(folder: &Path, services: &[Service], ledger: &Ledger) -> Vec<Outside> {
    let mut placements = ledger.lock();
    let Some(listing) = folder::list(folder, services, &placements) else {
        return Vec::new();
    };
    let mut changes = Vec::new();
    for listed in listing.blends {
        let unchanged = placements
            .get(&listed.blend.id)
            .is_some_and(|placed| placed.digest == listed.placed.digest);
        let id = listed.blend.id.clone();
        if !unchanged {
            changes.push(Outside::Edited(Arc::new(listed.blend)));
        }
        placements.insert(id, listed.placed);
    }
    if listing.whole {
        let removed: Vec<BlendId> = placements
            .keys()
            .filter(|id| !listing.present.contains(*id))
            .cloned()
            .collect();
        for id in removed {
            placements.remove(&id);
            changes.push(Outside::Gone(id));
        }
    }
    changes
}
