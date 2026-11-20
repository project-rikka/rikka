use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

pub(crate) type Value = Arc<dyn Any + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CacheKey {
    pub(crate) table: &'static str,
    pub(crate) id: Arc<str>,
}

impl CacheKey {
    pub(crate) fn new(table: &'static str, id: &str) -> CacheKey {
        CacheKey {
            table,
            id: Arc::from(id),
        }
    }
}

pub(crate) enum Lookup {
    Hit(Value),
    Removed,
    Miss,
}

pub(crate) enum Change {
    Put(CacheKey, Value),
    Remove(CacheKey),
    ClearTable(&'static str),
    PutBlob(CacheKey, Arc<[u8]>),
    RemoveBlob(CacheKey),
    Wipe,
}

struct Entry {
    value: Option<Value>,
    pending: Option<u64>,
    used: u64,
}

struct PendingBlob {
    bytes: Option<Arc<[u8]>>,
    seq: u64,
}

pub(crate) struct Overlay {
    entries: HashMap<CacheKey, Entry>,
    order: BTreeMap<u64, CacheKey>,
    blobs: HashMap<CacheKey, PendingBlob>,
    cleared: HashMap<&'static str, u64>,
    wiped: Option<u64>,
    capacity: usize,
    tick: u64,
    generation: u64,
}

pub(crate) struct Pending {
    pub(crate) hidden: bool,
    pub(crate) records: Vec<(Arc<str>, Option<Value>)>,
}

impl Overlay {
    pub(crate) fn new(capacity: usize) -> Overlay {
        Overlay {
            entries: HashMap::new(),
            order: BTreeMap::new(),
            blobs: HashMap::new(),
            cleared: HashMap::new(),
            wiped: None,
            capacity,
            tick: 0,
            generation: 0,
        }
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn get(&mut self, cache_key: &CacheKey) -> Lookup {
        self.tick += 1;
        let tick = self.tick;
        let Some(entry) = self.entries.get_mut(cache_key) else {
            return match self.is_hidden(cache_key.table) {
                true => Lookup::Removed,
                false => Lookup::Miss,
            };
        };
        if entry.pending.is_none() {
            self.order.remove(&entry.used);
            self.order.insert(tick, cache_key.clone());
        }
        entry.used = tick;
        match &entry.value {
            Some(value) => Lookup::Hit(Arc::clone(value)),
            None => Lookup::Removed,
        }
    }

    pub(crate) fn get_blob(&self, cache_key: &CacheKey) -> Option<Option<Arc<[u8]>>> {
        if let Some(pending) = self.blobs.get(cache_key) {
            return Some(pending.bytes.clone());
        }
        self.wiped.map(|_| None)
    }

    pub(crate) fn fill(&mut self, cache_key: CacheKey, value: Value, generation: u64) {
        if generation != self.generation || self.entries.contains_key(&cache_key) {
            return;
        }
        self.tick += 1;
        self.order.insert(self.tick, cache_key.clone());
        self.entries.insert(
            cache_key,
            Entry {
                value: Some(value),
                pending: None,
                used: self.tick,
            },
        );
        self.evict();
    }

    pub(crate) fn pending(&self, table: &'static str) -> Pending {
        Pending {
            hidden: self.is_hidden(table),
            records: self
                .entries
                .iter()
                .filter(|(cache_key, entry)| cache_key.table == table && entry.pending.is_some())
                .map(|(cache_key, entry)| (Arc::clone(&cache_key.id), entry.value.clone()))
                .collect(),
        }
    }

    pub(crate) fn apply(&mut self, change: Change, seq: u64) {
        self.generation += 1;
        match change {
            Change::Put(cache_key, value) => self.stage(cache_key, Some(value), seq),
            Change::Remove(cache_key) => self.stage(cache_key, None, seq),
            Change::PutBlob(cache_key, bytes) => {
                self.blobs.insert(
                    cache_key,
                    PendingBlob {
                        bytes: Some(bytes),
                        seq,
                    },
                );
            }
            Change::RemoveBlob(cache_key) => {
                self.blobs
                    .insert(cache_key, PendingBlob { bytes: None, seq });
            }
            Change::ClearTable(table) => {
                self.entries.retain(|cache_key, _| cache_key.table != table);
                self.order.retain(|_, cache_key| cache_key.table != table);
                self.cleared.insert(table, seq);
            }
            Change::Wipe => {
                self.entries.clear();
                self.order.clear();
                self.blobs.clear();
                self.cleared.clear();
                self.wiped = Some(seq);
            }
        }
    }

    pub(crate) fn commit_writes(&mut self, committed: &[Settled]) {
        self.generation += 1;
        for committed in committed {
            match committed {
                Settled::Record(cache_key, seq) => self.commit_record(cache_key, *seq),
                Settled::Blob(cache_key, seq) => {
                    if self
                        .blobs
                        .get(cache_key)
                        .is_some_and(|overlay| overlay.seq == *seq)
                    {
                        self.blobs.remove(cache_key);
                    }
                }
                Settled::Table(table, seq) => {
                    if self.cleared.get(table) == Some(seq) {
                        self.cleared.remove(table);
                    }
                }
                Settled::Wipe(seq) => {
                    if self.wiped == Some(*seq) {
                        self.wiped = None;
                    }
                }
            }
        }
        self.evict();
    }

    pub(crate) fn discard_writes(&mut self, dropped: &[Settled]) {
        self.generation += 1;
        for dropped in dropped {
            match dropped {
                Settled::Record(cache_key, seq) => {
                    if self
                        .entries
                        .get(cache_key)
                        .is_some_and(|entry| entry.pending == Some(*seq))
                    {
                        self.entries.remove(cache_key);
                    }
                }
                Settled::Blob(cache_key, seq) => {
                    if self
                        .blobs
                        .get(cache_key)
                        .is_some_and(|overlay| overlay.seq == *seq)
                    {
                        self.blobs.remove(cache_key);
                    }
                }
                Settled::Table(..) | Settled::Wipe(_) => {}
            }
        }
    }

    fn commit_record(&mut self, cache_key: &CacheKey, seq: u64) {
        let Some(entry) = self.entries.get_mut(cache_key) else {
            return;
        };
        if entry.pending != Some(seq) {
            return;
        }
        match entry.value {
            Some(_) => {
                entry.pending = None;
                self.order.insert(entry.used, cache_key.clone());
            }
            None => {
                self.entries.remove(cache_key);
            }
        }
    }

    fn stage(&mut self, cache_key: CacheKey, value: Option<Value>, seq: u64) {
        self.tick += 1;
        if let Some(entry) = self.entries.get(&cache_key)
            && entry.pending.is_none()
        {
            self.order.remove(&entry.used);
        }
        self.entries.insert(
            cache_key,
            Entry {
                value,
                pending: Some(seq),
                used: self.tick,
            },
        );
    }

    fn is_hidden(&self, table: &str) -> bool {
        self.wiped.is_some() || self.cleared.contains_key(table)
    }

    fn evict(&mut self) {
        while self.order.len() > self.capacity {
            let Some((_, cache_key)) = self.order.pop_first() else {
                return;
            };
            self.entries.remove(&cache_key);
        }
    }
}

pub(crate) enum Settled {
    Record(CacheKey, u64),
    Blob(CacheKey, u64),
    Table(&'static str, u64),
    Wipe(u64),
}
