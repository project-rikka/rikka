use rikka_sync::Mutex;
use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use rikka_picture::{Sleepers, Waking};

const LIMIT: usize = 1 << 16;

pub(crate) const LANGUAGE: u64 = u64::MAX;

const FETCHED: u64 = 1 << 63;

static KEYS: AtomicU64 = AtomicU64::new(1);
static CLOCK: AtomicU64 = AtomicU64::new(0);
static READERS: AtomicUsize = AtomicUsize::new(0);
static LANGUAGES: AtomicU64 = AtomicU64::new(0);
static LOG: Mutex<Log> = Mutex::new(Log::new());
static CHANGED: Sleepers = Sleepers::new();

thread_local! {
    static QUIET: Cell<u32> = const { Cell::new(0) };
}

#[derive(Clone, Copy)]
struct ReaderCursor {
    read: u64,
    overrun: bool,
}

struct Log {
    entries: VecDeque<(u64, u64)>,
    readers: Vec<Option<ReaderCursor>>,
}

impl Log {
    const fn new() -> Log {
        Log {
            entries: VecDeque::new(),
            readers: Vec::new(),
        }
    }

    fn push(&mut self, stamp: u64, key: u64) {
        match self.entries.back_mut() {
            Some(last) if last.1 == key => last.0 = stamp,
            _ => self.entries.push_back((stamp, key)),
        }
        if self.entries.len() > LIMIT {
            self.cut(self.entries.len() / 2);
        }
    }

    fn cut(&mut self, count: usize) {
        let Some(&(cut, _)) = self.entries.get(count.saturating_sub(1)) else {
            return;
        };
        self.entries.drain(..count);
        for reader in self.readers.iter_mut().flatten() {
            if reader.read < cut {
                reader.overrun = true;
                reader.read = cut;
            }
        }
    }

    fn truncate(&mut self) {
        let Some(oldest) = self
            .readers
            .iter()
            .flatten()
            .map(|reader| reader.read)
            .min()
        else {
            self.entries.clear();
            return;
        };
        let stale = self.entries.partition_point(|(stamp, _)| *stamp <= oldest);
        self.entries.drain(..stale);
    }

    fn since(&self, read: u64) -> impl Iterator<Item = &(u64, u64)> {
        let start = self.entries.partition_point(|(stamp, _)| *stamp <= read);
        self.entries.range(start..)
    }

    fn enroll(&mut self, read: u64) -> usize {
        let reader = Some(ReaderCursor {
            read,
            overrun: false,
        });
        match self.readers.iter().position(Option::is_none) {
            Some(slot) => {
                self.readers[slot] = reader;
                slot
            }
            None => {
                self.readers.push(reader);
                self.readers.len() - 1
            }
        }
    }

    fn pending(&mut self, slot: usize, mut is_watched: impl FnMut(u64) -> bool) -> bool {
        let Some(reader) = self.readers[slot] else {
            return false;
        };
        if reader.overrun || self.since(reader.read).any(|(_, key)| is_watched(*key)) {
            return true;
        }
        if let Some(&(last, _)) = self.entries.back()
            && last > reader.read
        {
            self.readers[slot] = Some(ReaderCursor {
                read: last,
                overrun: false,
            });
            self.truncate();
        }
        false
    }
}

pub(crate) fn key() -> u64 {
    KEYS.fetch_add(1, Ordering::Relaxed)
}

pub(crate) fn fetched(key: u64) -> u64 {
    key | FETCHED
}

pub(crate) fn is_fetched(key: u64) -> bool {
    key & FETCHED != 0 && key != LANGUAGE
}

pub(crate) fn now() -> u64 {
    CLOCK.load(Ordering::Acquire)
}

pub(crate) fn changed(key: u64) {
    if READERS.load(Ordering::Acquire) == 0 || QUIET.get() > 0 {
        return;
    }
    {
        let mut log = LOG.lock();
        let stamp = CLOCK.fetch_add(1, Ordering::AcqRel) + 1;
        log.push(stamp, key);
    }
    if !crate::task::handling() {
        CHANGED.wake_all();
    }
}

pub(crate) fn languages(generation: u64) {
    let current = LANGUAGES.load(Ordering::Acquire);
    if current != generation
        && LANGUAGES
            .compare_exchange(current, generation, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    {
        changed(LANGUAGE);
    }
}

pub(crate) fn on_change(wake: &Waking) {
    CHANGED.register(wake);
}

pub(crate) struct Quiet(());

impl Quiet {
    pub(crate) fn enter() -> Quiet {
        QUIET.set(QUIET.get() + 1);
        Quiet(())
    }
}

impl Drop for Quiet {
    fn drop(&mut self) {
        QUIET.set(QUIET.get() - 1);
    }
}

pub(crate) struct Reader {
    slot: usize,
}

impl Reader {
    pub(crate) fn new() -> Reader {
        let slot = LOG.lock().enroll(now());
        READERS.fetch_add(1, Ordering::AcqRel);
        Reader { slot }
    }

    pub(crate) fn pending(&self, is_watched: impl FnMut(u64) -> bool) -> bool {
        LOG.lock().pending(self.slot, is_watched)
    }

    pub(crate) fn take(&self) -> LogSlice {
        let mut log = LOG.lock();
        let at = now();
        let Some(reader) = log.readers[self.slot] else {
            return LogSlice {
                at,
                ..LogSlice::default()
            };
        };
        let changes = log.since(reader.read).copied().collect();
        log.readers[self.slot] = Some(ReaderCursor {
            read: at,
            overrun: false,
        });
        log.truncate();
        LogSlice {
            at,
            changes,
            overrun: reader.overrun,
        }
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        let mut log = LOG.lock();
        log.readers[self.slot] = None;
        READERS.fetch_sub(1, Ordering::AcqRel);
        log.truncate();
    }
}

impl std::fmt::Debug for Reader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reader").field("slot", &self.slot).finish()
    }
}

#[derive(Default)]
pub(crate) struct LogSlice {
    pub(crate) at: u64,
    pub(crate) changes: Vec<(u64, u64)>,
    pub(crate) overrun: bool,
}
