use std::collections::HashSet;
use std::rc::Rc;

use crate::history::History;
use crate::stored::StoredNotification;

#[derive(Debug)]
pub struct HistoryList {
    rows: Vec<HistoryRow>,
    next_key: u64,
}

#[derive(Debug)]
pub struct HistoryRow {
    pub key: u64,
    pub stored: Rc<StoredNotification>,
    pub arriving: bool,
    pub leaving: bool,
}

#[derive(Debug, Default)]
pub struct Followed {
    pub left: Vec<u64>,
    pub changed: bool,
}

impl HistoryList {
    #[must_use]
    pub fn new(history: &History) -> HistoryList {
        let mut list = HistoryList {
            rows: Vec::new(),
            next_key: 0,
        };
        list.rows = history.read(|entries| {
            entries
                .iter()
                .map(|stored| list.row(Rc::clone(stored), false))
                .collect()
        });
        list
    }

    #[must_use]
    pub fn rows(&self) -> &[HistoryRow] {
        &self.rows
    }

    #[must_use]
    pub fn has_standing(&self) -> bool {
        self.rows.iter().any(|row| !row.leaving)
    }

    #[must_use]
    pub fn follow(&mut self, history: &History) -> Followed {
        history.read(|entries| self.merge(entries))
    }

    pub fn remove(&mut self, key: u64) {
        self.rows.retain(|row| !(row.key == key && row.leaving));
    }

    fn merge(&mut self, entries: &[Rc<StoredNotification>]) -> Followed {
        let present: HashSet<u32> = entries.iter().map(|stored| stored.id()).collect();
        let mut arrived_early: HashSet<u32> = HashSet::new();
        let mut fresh = entries.iter().peekable();
        let mut merged = Vec::with_capacity(self.rows.len().max(entries.len()));
        let mut followed = Followed::default();
        for mut row in std::mem::take(&mut self.rows) {
            if row.leaving {
                merged.push(row);
                continue;
            }
            let id = row.stored.id();
            if !present.contains(&id) || arrived_early.contains(&id) {
                row.leaving = true;
                followed.left.push(row.key);
                merged.push(row);
                continue;
            }
            while let Some(arrived) = fresh.next_if(|stored| stored.id() != id) {
                arrived_early.insert(arrived.id());
                merged.push(self.row(Rc::clone(arrived), true));
                followed.changed = true;
            }
            if let Some(current) = fresh.next()
                && !Rc::ptr_eq(current, &row.stored)
            {
                row.stored = Rc::clone(current);
                followed.changed = true;
            }
            merged.push(row);
        }
        for arrived in fresh {
            merged.push(self.row(Rc::clone(arrived), true));
            followed.changed = true;
        }
        followed.changed |= !followed.left.is_empty();
        self.rows = merged;
        followed
    }

    fn row(&mut self, stored: Rc<StoredNotification>, arriving: bool) -> HistoryRow {
        let key = self.next_key;
        self.next_key += 1;
        HistoryRow {
            key,
            stored,
            arriving,
            leaving: false,
        }
    }
}
