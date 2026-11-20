use std::hash::Hash;

use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct LruCache<K, V> {
    entries: FxHashMap<K, Entry<V>>,
    total_bytes: usize,
    clock: u64,
    budget: usize,
}

#[derive(Debug)]
struct Entry<V> {
    value: V,
    bytes: usize,
    used: u64,
}

impl<K: Copy + Eq + Hash, V> LruCache<K, V> {
    #[must_use]
    pub fn new(budget: usize) -> Self {
        Self {
            entries: FxHashMap::default(),
            total_bytes: 0,
            clock: 0,
            budget,
        }
    }

    pub fn get(&mut self, key: K) -> Option<&V> {
        self.clock += 1;
        let clock = self.clock;
        let entry = self.entries.get_mut(&key)?;
        entry.used = clock;
        Some(&entry.value)
    }

    pub fn insert(&mut self, key: K, value: V, bytes: usize) {
        self.clock += 1;
        let entry = Entry {
            value,
            bytes,
            used: self.clock,
        };
        if let Some(evicted) = self.entries.insert(key, entry) {
            self.total_bytes -= evicted.bytes;
        }
        self.total_bytes += bytes;
        self.evict();
    }

    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    fn evict(&mut self) {
        if self.total_bytes <= self.budget {
            return;
        }
        let mut order: Vec<(u64, K)> = self
            .entries
            .iter()
            .map(|(key, entry)| (entry.used, *key))
            .collect();
        order.sort_unstable_by_key(|(used, _)| *used);
        let target = self.budget - self.budget / 4;
        for (_, key) in order {
            if self.total_bytes <= target {
                return;
            }
            if let Some(evicted) = self.entries.remove(&key) {
                self.total_bytes -= evicted.bytes;
            }
        }
    }
}
