use std::rc::Rc;

use rikka_ui::hash_of;

use crate::processes::row::Row;
use crate::processes::table::Snapshot;

pub(crate) struct RowOrder {
    pub(crate) snapshot: Rc<Snapshot>,
    pub(crate) glides: bool,
    pub(crate) order: u64,
}

impl RowOrder {
    pub(crate) fn new(snapshot: Rc<Snapshot>, glides: bool) -> RowOrder {
        let names: Vec<u64> = snapshot.rows.iter().map(Row::name).collect();
        RowOrder {
            snapshot,
            glides,
            order: hash_of(names),
        }
    }
}

impl PartialEq for RowOrder {
    fn eq(&self, other: &Self) -> bool {
        self.glides == other.glides && self.order == other.order
    }
}
