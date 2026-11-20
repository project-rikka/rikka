use std::hash::Hash;

use rustc_hash::FxHashMap;

use crate::error::Result;
use crate::outline::prepared::PreparedOutline;

const DEFAULT_BUDGET: usize = 8 << 20;

pub struct OutlineCache<K> {
    slots: Vec<Slot<K>>,
    index: FxHashMap<K, usize>,
    hand: usize,
    bytes: usize,
    budget: usize,
}

struct Slot<K> {
    key: K,
    outline: PreparedOutline,
    recent: bool,
    bytes: usize,
}

impl<K> Default for OutlineCache<K> {
    fn default() -> Self {
        Self::with_budget(DEFAULT_BUDGET)
    }
}

impl<K> OutlineCache<K> {
    pub fn with_budget(bytes: usize) -> Self {
        Self {
            slots: Vec::new(),
            index: FxHashMap::default(),
            hand: 0,
            bytes: 0,
            budget: bytes,
        }
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

impl<K: Copy + Eq + Hash> OutlineCache<K> {
    pub fn outline(
        &mut self,
        key: K,
        prepare: impl FnOnce(&mut PreparedOutline) -> Result<()>,
    ) -> Result<&mut PreparedOutline> {
        if let Some(&at) = self.index.get(&key) {
            self.update_size(at);
            let slot = &mut self.slots[at];
            slot.recent = true;
            return Ok(&mut slot.outline);
        }
        self.evict_over_budget();
        let mut outline = PreparedOutline::new();
        prepare(&mut outline)?;
        let at = self.slots.len();
        self.slots.push(Slot {
            key,
            outline,
            recent: true,
            bytes: 0,
        });
        self.index.insert(key, at);
        self.update_size(at);
        Ok(&mut self.slots[at].outline)
    }

    fn update_size(&mut self, at: usize) {
        let slot = &mut self.slots[at];
        let bytes = slot.outline.heap_bytes() + size_of::<Slot<K>>();
        self.bytes = self.bytes - slot.bytes + bytes;
        slot.bytes = bytes;
    }

    fn evict_over_budget(&mut self) {
        while self.bytes > self.budget && !self.slots.is_empty() {
            let at = self.hand % self.slots.len();
            if self.slots[at].recent {
                self.slots[at].recent = false;
                self.hand = at + 1;
                continue;
            }
            let evicted = self.slots.swap_remove(at);
            self.index.remove(&evicted.key);
            self.bytes -= evicted.bytes;
            if let Some(moved) = self.slots.get(at) {
                self.index.insert(moved.key, at);
            }
            self.hand = at;
        }
    }
}

impl<K> std::fmt::Debug for OutlineCache<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutlineCache")
            .field("outlines", &self.slots.len())
            .field("bytes", &self.bytes)
            .field("budget", &self.budget)
            .finish()
    }
}
