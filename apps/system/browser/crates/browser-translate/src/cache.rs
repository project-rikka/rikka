use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use crate::call::Translated;

const ENTRY_OVERHEAD: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CacheKey {
    pub(crate) service: &'static str,
    pub(crate) to: &'static str,
    pub(crate) protect: bool,
    pub(crate) html: String,
}

#[derive(Debug)]
pub(crate) struct Cache {
    entries: HashMap<Arc<CacheKey>, Entry>,
    order: BTreeMap<u64, Arc<CacheKey>>,
    bytes: usize,
    budget: usize,
    tick: u64,
}

#[derive(Debug)]
struct Entry {
    value: Arc<Translated>,
    tick: u64,
    size: usize,
}

impl Cache {
    pub(crate) fn new(budget: usize) -> Cache {
        Cache {
            entries: HashMap::new(),
            order: BTreeMap::new(),
            bytes: 0,
            budget,
            tick: 0,
        }
    }

    pub(crate) fn get(&mut self, key: &CacheKey) -> Option<Arc<Translated>> {
        self.tick += 1;
        let tick = self.tick;
        let (shared, entry) = self.entries.get_key_value(key)?;
        let shared = Arc::clone(shared);
        let value = Arc::clone(&entry.value);
        let previous = entry.tick;
        self.order.remove(&previous);
        self.order.insert(tick, shared);
        if let Some(entry) = self.entries.get_mut(key) {
            entry.tick = tick;
        }
        Some(value)
    }

    pub(crate) fn insert(&mut self, key: CacheKey, value: Arc<Translated>) {
        let size = key.html.len()
            + value.text.len()
            + value.detected.as_ref().map_or(0, String::len)
            + ENTRY_OVERHEAD;
        if size > self.budget {
            return;
        }
        self.remove(&key);
        self.tick += 1;
        let key = Arc::new(key);
        self.order.insert(self.tick, Arc::clone(&key));
        self.entries.insert(
            key,
            Entry {
                value,
                tick: self.tick,
                size,
            },
        );
        self.bytes += size;
        while self.bytes > self.budget {
            let Some((_, oldest)) = self.order.pop_first() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.bytes -= entry.size;
            }
        }
    }

    fn remove(&mut self, key: &CacheKey) {
        if let Some(entry) = self.entries.remove(key) {
            self.order.remove(&entry.tick);
            self.bytes -= entry.size;
        }
    }
}
