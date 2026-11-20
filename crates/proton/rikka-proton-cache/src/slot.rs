use rikka_sync::Mutex;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cache::Cache;
use crate::table::{Record, Table};

pub struct Slot<T> {
    cache: Cache,
    table: Table<T>,
    id: Arc<str>,
    generation: Arc<Generation>,
}

#[derive(Default)]
struct Generation {
    latest: AtomicU64,
    write_lock: Mutex<()>,
}

#[must_use]
pub struct Staged<T> {
    slot: Slot<T>,
    generation: u64,
    value: T,
}

impl<T: Record> Slot<T> {
    pub(crate) fn new(cache: Cache, table: Table<T>, id: &str) -> Slot<T> {
        Slot {
            cache,
            table,
            id: Arc::from(id),
            generation: Arc::default(),
        }
    }

    #[must_use]
    pub fn read(&self) -> Option<T> {
        self.cache.get(&self.table, &self.id)
    }

    pub fn stage(&self, value: T) -> Staged<T> {
        let generation = self.generation.latest.fetch_add(1, Ordering::SeqCst) + 1;
        Staged {
            slot: self.clone(),
            generation,
            value,
        }
    }

    pub fn forget(&self) {
        self.generation.latest.fetch_add(1, Ordering::SeqCst);
        self.cache.remove(&self.table, &self.id);
    }
}

impl<T: Record> Staged<T> {
    pub fn write(self) {
        let generation = &self.slot.generation;
        let _guard = generation.write_lock.lock();
        if generation.latest.load(Ordering::SeqCst) != self.generation {
            return;
        }
        self.slot
            .cache
            .put(&self.slot.table, &self.slot.id, self.value);
    }
}

impl<T> Clone for Slot<T> {
    fn clone(&self) -> Slot<T> {
        Slot {
            cache: self.cache.clone(),
            table: self.table,
            id: Arc::clone(&self.id),
            generation: Arc::clone(&self.generation),
        }
    }
}

impl<T> fmt::Debug for Slot<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Slot")
            .field("table", &self.table.name())
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl<T> fmt::Debug for Staged<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Staged")
            .field("slot", &self.slot)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}
