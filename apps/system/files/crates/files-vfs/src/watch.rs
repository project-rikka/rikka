use std::path::Path;
use std::sync::Arc;

use files_core::Location;
use rikka_watch::{Event, Reach, Watcher};

use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Listing { at: Location },
    Entry { at: Location },
}

pub type WatchSink = Arc<dyn Fn(Change) + Send + Sync>;

#[derive(Debug)]
pub struct Watch {
    _watcher: Option<Watcher>,
}

impl Watch {
    #[must_use]
    pub fn inert() -> Self {
        Self { _watcher: None }
    }

    pub(crate) fn directory(path: &Path, at: &Location, sink: WatchSink) -> Result<Self> {
        let here = at.clone();
        let watcher = Watcher::spawn(move |event| sink(classify(event, &here)))
            .map_err(|error| watch_failed(error, at))?;
        watcher
            .watch(path, Reach::Contents)
            .map_err(|error| watch_failed(error, at))?;

        Ok(Self {
            _watcher: Some(watcher),
        })
    }
}

fn classify(event: Event, at: &Location) -> Change {
    match event {
        Event::Changed(change) if !change.kind.alters_listing() => Change::Entry {
            at: Location::from_path(change.path),
        },
        Event::Changed(_) | Event::Lost => Change::Listing { at: at.clone() },
    }
}

fn watch_failed(error: rikka_watch::Error, at: &Location) -> Error {
    match error {
        rikka_watch::Error::NotFound(_) => Error::NotFound { at: at.clone() },
        rikka_watch::Error::Watch { source, .. } => Error::from_io(&source, at),
        other => Error::Other {
            at: at.clone(),
            message: other.to_string(),
        },
    }
}
