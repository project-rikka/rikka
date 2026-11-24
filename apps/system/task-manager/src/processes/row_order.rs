use std::rc::Rc;

use rikka_ui::hash_of;
use rikka_widgets::Rearrange;

use crate::processes::row::Row;
use crate::processes::table::Snapshot;

pub(crate) struct RowOrder {
    pub(crate) snapshot: Rc<Snapshot>,
    pub(crate) rearrange: Rearrange,
    pub(crate) order: u64,
}

impl RowOrder {
    pub(crate) fn new(snapshot: Rc<Snapshot>, rearrange: Rearrange) -> RowOrder {
        let names: Vec<u64> = snapshot.rows.iter().map(Row::name).collect();
        RowOrder {
            snapshot,
            rearrange,
            order: hash_of(names),
        }
    }
}

impl PartialEq for RowOrder {
    fn eq(&self, other: &Self) -> bool {
        self.rearrange == other.rearrange && self.order == other.order
    }
}
