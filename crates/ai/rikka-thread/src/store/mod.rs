mod crew;
mod format;
mod journal;
mod name;
mod writer;

use rikka_sync::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_file_edit::History;

use crate::error::{Error, Result};
use crate::thread::{Summary, Thread};

pub use crew::Crew;
pub(crate) use crew::{Member, Standing};
pub(crate) use format::{BLOBS, JOURNAL, Line, restored};
pub use name::named;

use format::{META, read_summary};
use writer::{Entry, Keeper, WriterState};

const CHECKPOINTS: &str = "checkpoints";

#[derive(Clone, Debug)]
pub struct Store {
    keeper: Arc<Keeper>,
    histories: Arc<Mutex<HashMap<String, History>>>,
    crews: Arc<Mutex<HashMap<String, Crew>>>,
}

impl Store {
    pub fn open(folder: &Path) -> Result<Store> {
        let folder = folder.to_path_buf();
        std::fs::create_dir_all(&folder).map_err(|error| Error::Unwritable {
            path: folder.display().to_string(),
            reason: error.to_string(),
        })?;
        Ok(Store::new(folder))
    }

    #[must_use]
    pub fn summaries(&self) -> Vec<Summary> {
        let folder = &self.writer_state().folder;
        let listing = match std::fs::read_dir(folder) {
            Ok(listing) => listing,
            Err(error) => {
                tracing::warn!("failed to read {}: {error}", folder.display());
                return Vec::new();
            }
        };
        let mut summaries: Vec<Summary> = listing
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                if !path.is_dir() {
                    return None;
                }
                let at = path.join(META);
                match read_summary(&at) {
                    Ok(summary) => Some(summary),
                    Err(error) => {
                        tracing::warn!("{}: {error}", at.display());
                        None
                    }
                }
            })
            .collect();
        summaries.sort_by_key(|summary| std::cmp::Reverse(summary.updated()));
        summaries
    }

    pub fn read(&self, id: &str) -> Result<Thread> {
        self.writer_state().read(id)
    }

    pub fn keep(&self, thread: &Thread) {
        self.writer_state()
            .put(thread.id(), Entry::Thread(thread.clone()));
    }

    pub fn discard(&self, id: &str) {
        self.histories.lock().remove(id);
        self.crews.lock().remove(id);
        self.writer_state().put(id, Entry::Gone);
    }

    #[must_use]
    pub fn history(&self, id: &str) -> History {
        let folder = self.writer_state().folder.join(id).join(CHECKPOINTS);
        self.histories
            .lock()
            .entry(id.to_owned())
            .or_insert_with(|| History::open(folder))
            .clone()
    }

    #[must_use]
    pub fn crew(&self, id: &str) -> Crew {
        self.crews
            .lock()
            .entry(id.to_owned())
            .or_insert_with(|| Crew::new(Arc::clone(&self.keeper.store), id))
            .clone()
    }

    pub fn watch(&self, on_error: impl Fn(Error) + Send + Sync + 'static) {
        self.writer_state().watch(Box::new(on_error));
    }

    #[must_use]
    pub fn new(folder: impl Into<PathBuf>) -> Store {
        Store {
            keeper: Arc::new(Keeper::new(folder.into())),
            histories: Arc::default(),
            crews: Arc::default(),
        }
    }

    fn writer_state(&self) -> &WriterState {
        &self.keeper.store
    }
}
