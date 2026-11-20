use rikka_sync::{Condvar, Mutex, MutexGuard};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::thread::JoinHandle;

use super::format::{BLOBS, JOURNAL, Line, META, encode_line, read_summary, stored, write_blob};
use super::journal::{appended, line_of, read_journal, rewritten};
use crate::error::{Error, Result};
use crate::journal::Journal;
use crate::thread::{Summary, Thread};
use crate::turn::Turn;

pub(super) const SLACK: usize = 64 * 1024;

#[derive(Debug)]
pub(super) struct Keeper {
    pub(super) store: Arc<WriterState>,
    worker: Option<JoinHandle<()>>,
}

impl Keeper {
    pub(super) fn new(folder: PathBuf) -> Keeper {
        let store = Arc::new(WriterState::new(folder));
        let keeping = Arc::clone(&store);
        match std::thread::Builder::new()
            .name("rikka-thread-writer".to_owned())
            .spawn(move || keeping.keep())
        {
            Ok(worker) => Keeper {
                store,
                worker: Some(worker),
            },
            Err(error) => {
                tracing::error!(%error, "failed to spawn thread writer, writing synchronously");
                store.queue().keeping = false;
                Keeper {
                    store,
                    worker: None,
                }
            }
        }
    }
}

impl Drop for Keeper {
    fn drop(&mut self) {
        self.store.close();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            tracing::error!("thread writer panicked");
        }
    }
}

pub(super) struct WriterState {
    pub(super) folder: PathBuf,
    queue: Mutex<Queue>,
    progress: Mutex<BTreeMap<String, Progress>>,
    gone: Mutex<BTreeSet<String>>,
    watcher: Mutex<Option<Watcher>>,
    knock: Condvar,
}

impl std::fmt::Debug for WriterState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Kept")
            .field("folder", &self.folder)
            .finish_non_exhaustive()
    }
}

pub(super) type Watcher = Box<dyn Fn(Error) + Send + Sync>;

#[derive(Debug)]
pub(super) enum Entry {
    Thread(Thread),
    Document(Vec<u8>),
    Gone,
}

#[derive(Debug)]
struct Queue {
    waiting: BTreeMap<String, Entry>,
    keeping: bool,
    closed: bool,
}

#[derive(Debug, Default)]
pub(super) struct Progress {
    pub(super) turns: usize,
    last: Option<Weak<Turn>>,
    running: bool,
    meta: Option<Summary>,
    pub(super) committed: usize,
    pub(super) journal: usize,
    journal_file: Option<Arc<Journal>>,
}

enum Course<'a> {
    Whole,
    More {
        turns: &'a [Arc<Turn>],
        running: Option<&'a Arc<Turn>>,
        shed: bool,
    },
}

impl WriterState {
    fn new(folder: PathBuf) -> WriterState {
        WriterState {
            folder,
            queue: Mutex::new(Queue {
                waiting: BTreeMap::new(),
                keeping: true,
                closed: false,
            }),
            progress: Mutex::new(BTreeMap::new()),
            gone: Mutex::new(BTreeSet::new()),
            watcher: Mutex::new(None),
            knock: Condvar::new(),
        }
    }

    pub(super) fn watch(&self, watcher: Watcher) {
        *self.watcher.lock() = Some(watcher);
    }

    pub(super) fn put(&self, id: &str, entry: Entry) {
        let mut queue = self.queue();
        if !queue.keeping {
            drop(queue);
            self.write(id, &entry);
            return;
        }
        queue.waiting.insert(id.to_owned(), entry);
        self.knock.notify_one();
    }

    fn keep(&self) {
        loop {
            let batch = {
                let mut queue = self.queue();
                while queue.waiting.is_empty() && !queue.closed {
                    queue = self.knock.wait(queue);
                }
                if queue.waiting.is_empty() {
                    return;
                }
                std::mem::take(&mut queue.waiting)
            };

            for (id, entry) in batch {
                self.write(&id, &entry);
            }
        }
    }

    fn close(&self) {
        self.queue().closed = true;
        self.knock.notify_all();
    }

    pub(super) fn read(&self, id: &str) -> Result<Thread> {
        let at = self.folder.join(id);
        let snapshot = self.progress();
        let meta = at.join(META);
        let summary = read_summary(&meta).map_err(|reason| Error::Unreadable {
            path: meta.display().to_string(),
            reason,
        })?;
        let (turns, _) = read_journal(&at).map_err(|error| Error::Unreadable {
            path: at.join(JOURNAL).display().to_string(),
            reason: error.to_string(),
        })?;
        drop(snapshot);
        Ok(Thread::from_turns(summary, turns))
    }

    fn write(&self, id: &str, entry: &Entry) {
        let at = self.folder.join(id);
        let document = match entry {
            _ if self.is_gone(id) => Ok(()),
            Entry::Thread(thread) => self.written_down(&at, thread),
            Entry::Document(bytes) => written(&at, bytes),
            Entry::Gone => self.removed(&at, id),
        };
        let Err(error) = document else {
            return;
        };
        tracing::error!("failed to write {}: {error}", at.display());
        let watcher = self.watcher.lock();
        if let Some(watcher) = watcher.as_ref() {
            watcher(Error::Unwritable {
                path: at.display().to_string(),
                reason: error.to_string(),
            });
        }
    }

    fn is_gone(&self, id: &str) -> bool {
        let gone = self.gone.lock();
        Path::new(id)
            .ancestors()
            .skip(1)
            .any(|above| gone.contains(above.to_string_lossy().as_ref()))
    }

    fn written_down(&self, at: &Path, thread: &Thread) -> std::io::Result<()> {
        let blobs = at.join(BLOBS);
        std::fs::create_dir_all(&blobs)?;
        let mut all = self.progress();
        let progress = all.entry(thread.id().to_owned()).or_default();
        let journal = Arc::clone(
            progress
                .journal_file
                .get_or_insert_with(|| Journal::open(at)),
        );

        match course(progress, thread) {
            Course::Whole => rewritten(at, thread, progress, &journal)?,
            Course::More {
                turns,
                running,
                shed,
            } => {
                let mut lines: Vec<(Option<&Arc<Turn>>, Vec<u8>)> =
                    Vec::with_capacity(turns.len() + 1);
                for turn in turns {
                    lines.push((Some(turn), line_of(turn, &blobs, &journal)?));
                }
                if let Some(running) = running {
                    lines.push((None, encode_line(&Line::Running(stored(running, &blobs)?))?));
                } else if shed {
                    lines.push((None, encode_line(&Line::Shed)?));
                }
                if !lines.is_empty() {
                    appended(at, &lines, progress, &journal)?;
                }
                if progress.journal > progress.committed.saturating_mul(2).saturating_add(SLACK) {
                    rewritten(at, thread, progress, &journal)?;
                }
            }
        }

        let settled_turns = settled_count(thread);
        progress.turns = settled_turns;
        progress.last = thread.all_turns()[..settled_turns]
            .last()
            .map(Arc::downgrade);
        progress.running = thread.is_running();
        let summary = thread.summary();
        if progress.meta.as_ref() != Some(&summary) {
            let encoded = serde_json::to_vec_pretty(&summary).map_err(std::io::Error::other)?;
            write_blob(&at.join(META), &encoded)?;
            progress.meta = Some(summary);
        }
        Ok(())
    }

    fn removed(&self, at: &Path, id: &str) -> std::io::Result<()> {
        self.progress()
            .retain(|kept, _| !Path::new(kept).starts_with(id));
        self.gone.lock().insert(id.to_owned());
        match std::fs::remove_dir_all(at) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }

    fn queue(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock()
    }

    fn progress(&self) -> MutexGuard<'_, BTreeMap<String, Progress>> {
        self.progress.lock()
    }
}

fn written(at: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(folder) = at.parent() {
        std::fs::create_dir_all(folder)?;
    }
    write_blob(at, bytes)
}

fn course<'a>(progress: &Progress, thread: &'a Thread) -> Course<'a> {
    if progress.meta.is_none() {
        return Course::Whole;
    }
    let turns = thread.all_turns();
    let settled_turns = settled_count(thread);
    if settled_turns < progress.turns || !follows(progress, turns) {
        return Course::Whole;
    }
    Course::More {
        turns: &turns[progress.turns..settled_turns],
        running: turns.get(settled_turns),
        shed: progress.running && turns.len() == progress.turns,
    }
}

fn follows(progress: &Progress, turns: &[Arc<Turn>]) -> bool {
    let Some(last) = progress.last.as_ref() else {
        return progress.turns == 0;
    };
    turns
        .get(progress.turns.wrapping_sub(1))
        .is_some_and(|now| std::ptr::eq(last.as_ptr(), Arc::as_ptr(now)))
}

pub(super) fn settled_count(thread: &Thread) -> usize {
    thread.all_turns().len() - usize::from(thread.is_running())
}
