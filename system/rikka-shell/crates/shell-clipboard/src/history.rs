use std::sync::Arc;

use rikka_sync::{Mutex, MutexGuard};

use crate::entry::{Content, Entry};

pub(super) const MAX_CLIPPING: usize = 32 * 1024 * 1024;

const BUDGET: usize = 192 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct History(Arc<Mutex<Store>>);

#[derive(Debug)]
struct Store {
    entries: Vec<Entry>,
    next: u64,
    capacity: usize,
    budget: usize,
    refused: bool,
}

impl History {
    #[must_use]
    pub(crate) fn new(capacity: usize) -> History {
        History::with_limits(capacity, BUDGET)
    }

    #[must_use]
    fn with_limits(capacity: usize, budget: usize) -> History {
        History(Arc::new(Mutex::new(Store {
            entries: Vec::new(),
            next: 1,
            capacity,
            budget,
            refused: false,
        })))
    }

    pub(super) fn set_capacity(&self, capacity: usize) {
        let mut store = self.store();
        store.capacity = capacity;
        store.forget();
    }

    pub(crate) fn record(&self, content: Content) {
        let mut store = self.store();
        if let Some(at) = store
            .entries
            .iter()
            .position(|entry| entry.content.is(&content))
        {
            let known = store.entries.remove(at);
            store.entries.insert(0, known);
            return;
        }
        let id = store.next;
        let candidate = Entry::new(id, content);
        if !store.has_room_for(candidate.weight()) {
            store.refused = true;
            tracing::warn!(
                "skipped clipboard entry: pinned entries use {} of {} bytes",
                store.pinned_weight(),
                store.budget
            );
            return;
        }
        store.next += 1;
        store.entries.insert(0, candidate);
        store.forget();
    }

    #[must_use]
    pub(crate) fn snapshot(&self) -> Vec<Entry> {
        let mut entries = self.store().entries.clone();
        entries.sort_by_key(|entry| !entry.pinned);
        entries
    }

    #[must_use]
    pub(crate) fn get(&self, id: u64) -> Option<Entry> {
        self.store()
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
    }

    #[must_use]
    pub(super) fn turned_away(&self) -> bool {
        self.store().refused
    }

    pub(crate) fn remove(&self, id: u64) {
        let mut store = self.store();
        store.entries.retain(|entry| entry.id != id);
        store.refused = false;
    }

    pub(crate) fn pin(&self, id: u64, pinned: bool) {
        let mut store = self.store();
        if let Some(entry) = store.entries.iter_mut().find(|entry| entry.id == id) {
            entry.pinned = pinned;
        }
        if !pinned {
            store.refused = false;
        }
        store.forget();
    }

    pub(crate) fn clear(&self) {
        let mut store = self.store();
        store.entries.retain(|entry| entry.pinned);
        store.refused = false;
    }

    fn store(&self) -> MutexGuard<'_, Store> {
        self.0.lock()
    }
}

impl Store {
    fn forget(&mut self) {
        while self.is_over_budget() {
            let Some(oldest) = self
                .entries
                .iter()
                .rposition(|entry| !entry.pinned)
                .filter(|at| *at > 0)
            else {
                return;
            };
            self.entries.remove(oldest);
        }
    }

    fn is_over_budget(&self) -> bool {
        let candidate = self.entries.iter().filter(|entry| !entry.pinned).count();
        candidate > self.capacity || self.weight() > self.budget
    }

    fn has_room_for(&self, bytes: usize) -> bool {
        self.pinned_weight().saturating_add(bytes) <= self.budget
    }

    fn weight(&self) -> usize {
        self.entries.iter().map(|entry| entry.weight()).sum()
    }

    fn pinned_weight(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.pinned)
            .map(|entry| entry.weight())
            .sum()
    }
}
