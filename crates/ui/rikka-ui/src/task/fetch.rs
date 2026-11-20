use std::any::Any;
use std::future::Future;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use rikka_sync::{Mutex, MutexGuard};
use rustc_hash::FxHashMap;
use tokio::task::JoinHandle;

use crate::view::hash_of;
use crate::view::observe::{log, reads};

const IDLE: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub enum Fetch<T> {
    Loading,
    Ready(Arc<T>),
    Failed(Arc<str>),
}

impl<T> Clone for Fetch<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Loading => Self::Loading,
            Self::Ready(value) => Self::Ready(Arc::clone(value)),
            Self::Failed(why) => Self::Failed(Arc::clone(why)),
        }
    }
}

impl<T> Fetch<T> {
    pub fn ready(&self) -> Option<&T> {
        match self {
            Self::Ready(value) => Some(value),
            _ => None,
        }
    }

    pub fn failed(&self) -> Option<&str> {
        match self {
            Self::Failed(why) => Some(why),
            _ => None,
        }
    }

    pub fn loading(&self) -> bool {
        matches!(self, Self::Loading)
    }
}

type Shared = Arc<dyn Any + Send + Sync>;

enum State {
    Pending(Option<JoinHandle<()>>),
    Ready(Shared),
    Failed(Arc<str>),
}

struct Entry {
    state: State,
    requested_at: Instant,
}

enum Hold {
    Read(usize),
    Left(Instant),
}

struct Table {
    entries: FxHashMap<u64, Entry>,
    holds: FxHashMap<u64, Hold>,
    swept: Instant,
}

impl Table {
    fn new(now: Instant) -> Table {
        Table {
            entries: FxHashMap::default(),
            holds: FxHashMap::default(),
            swept: now,
        }
    }

    fn hold(&mut self, read: u64) {
        match self.holds.get_mut(&read) {
            Some(Hold::Read(readers)) => *readers += 1,
            _ => {
                self.holds.insert(read, Hold::Read(1));
            }
        }
    }

    fn release(&mut self, read: u64, now: Instant) {
        let Some(hold) = self.holds.get_mut(&read) else {
            return;
        };
        match hold {
            Hold::Read(readers) if *readers > 1 => *readers -= 1,
            Hold::Read(_) => *hold = Hold::Left(now),
            Hold::Left(_) => {}
        }
    }

    fn sweep(&mut self, now: Instant) {
        if now.saturating_duration_since(self.swept) < IDLE {
            return;
        }
        self.swept = now;
        let recent = |at: Instant| now.saturating_duration_since(at) < IDLE;
        self.holds.retain(|_, hold| match hold {
            Hold::Read(_) => true,
            Hold::Left(at) => recent(*at),
        });
        let holds = &self.holds;
        self.entries.retain(|key, entry| {
            if recent(entry.requested_at) || holds.contains_key(&log::fetched(*key)) {
                return true;
            }
            if let State::Pending(Some(handle)) = &entry.state {
                handle.abort();
            }
            false
        });
    }
}

struct Fetches {
    table: Mutex<Table>,
}

static FETCHES: OnceLock<Fetches> = OnceLock::new();

fn fetches() -> &'static Fetches {
    FETCHES.get_or_init(|| Fetches {
        table: Mutex::new(Table::new(Instant::now())),
    })
}

fn table() -> MutexGuard<'static, Table> {
    fetches().table.lock()
}

pub(crate) fn hold(reads: impl IntoIterator<Item = u64>) {
    let mut reads = reads
        .into_iter()
        .filter(|read| log::is_fetched(*read))
        .peekable();
    if reads.peek().is_none() {
        return;
    }
    let mut table = table();
    reads.for_each(|read| table.hold(read));
}

pub(crate) fn release(reads: impl IntoIterator<Item = u64>) {
    let mut reads = reads
        .into_iter()
        .filter(|read| log::is_fetched(*read))
        .peekable();
    if reads.peek().is_none() {
        return;
    }
    let now = Instant::now();
    let mut table = table();
    reads.for_each(|read| table.release(read, now));
}

fn complete(key: u64, state: State) {
    {
        let mut table = table();
        let Some(entry) = table.entries.get_mut(&key) else {
            return;
        };
        entry.state = state;
    }
    log::changed(log::fetched(key));
}

pub(crate) fn keyed<T: 'static>(key: impl std::hash::Hash) -> u64 {
    hash_of((std::any::TypeId::of::<T>(), key))
}

fn answered<T: Send + Sync + 'static>(state: &State) -> Fetch<T> {
    match state {
        State::Pending(_) => Fetch::Loading,
        State::Failed(why) => Fetch::Failed(Arc::clone(why)),
        State::Ready(value) => match Arc::clone(value).downcast::<T>() {
            Ok(value) => Fetch::Ready(value),
            Err(_) => unreachable!("fetch result type mismatch"),
        },
    }
}

pub fn fetch<T, E, F>(key: impl std::hash::Hash, load: impl FnOnce() -> F) -> Fetch<T>
where
    T: Send + Sync + 'static,
    E: std::fmt::Display + Send + 'static,
    F: Future<Output = Result<T, E>> + Send + 'static,
{
    let key = keyed::<T>(key);
    started(key, || {
        let work = load();
        crate::task::work_runtime().spawn(async move {
            complete(key, outcome(work.await));
        })
    })
}

pub fn fetch_blocking<T, E>(
    key: impl std::hash::Hash,
    work: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Fetch<T>
where
    T: Send + Sync + 'static,
    E: std::fmt::Display + Send + 'static,
{
    let key = keyed::<T>(key);
    started(key, || {
        crate::task::work_runtime().spawn_blocking(move || {
            complete(key, outcome(work()));
        })
    })
}

fn outcome<T: Send + Sync + 'static, E: std::fmt::Display>(answer: Result<T, E>) -> State {
    match answer {
        Ok(value) => State::Ready(Arc::new(value)),
        Err(why) => State::Failed(Arc::from(why.to_string())),
    }
}

fn started<T: Send + Sync + 'static>(key: u64, start: impl FnOnce() -> JoinHandle<()>) -> Fetch<T> {
    reads::read(log::fetched(key));
    let now = Instant::now();
    {
        let mut table = table();
        table.sweep(now);
        if let Some(entry) = table.entries.get_mut(&key) {
            entry.requested_at = now;
            return answered(&entry.state);
        }
        table.entries.insert(
            key,
            Entry {
                state: State::Pending(None),
                requested_at: now,
            },
        );
    }

    let handle = start();
    let mut table = table();
    let Some(entry) = table.entries.get_mut(&key) else {
        handle.abort();
        return Fetch::Loading;
    };
    if let State::Pending(slot @ None) = &mut entry.state {
        *slot = Some(handle);
    }
    entry.requested_at = now;
    answered(&entry.state)
}
