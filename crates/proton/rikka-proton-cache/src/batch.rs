use std::fmt;

use crate::cache::{Cache, PendingWrite};
use crate::table::{Record, Table};

#[must_use]
pub struct Batch {
    cache: Cache,
    writes: Vec<PendingWrite>,
}

impl Batch {
    pub(crate) fn new(cache: Cache) -> Batch {
        Batch {
            cache,
            writes: Vec::new(),
        }
    }

    pub fn put<T: Record>(&mut self, table: &Table<T>, id: &str, value: T) -> &mut Batch {
        if let Some(writes) = self.cache.staged_put(table, id, value) {
            self.writes.push(writes);
        }
        self
    }

    pub fn remove<T: Record>(&mut self, table: &Table<T>, id: &str) -> &mut Batch {
        self.writes.push(self.cache.staged_remove(table, id));
        self
    }

    pub fn clear<T: Record>(&mut self, table: &Table<T>) -> &mut Batch {
        self.writes.push(self.cache.staged_clear(table));
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.writes.is_empty()
    }

    pub fn commit(self) {
        self.cache.send(self.writes);
    }
}

impl fmt::Debug for Batch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Batch")
            .field("changes", &self.writes.len())
            .finish_non_exhaustive()
    }
}
