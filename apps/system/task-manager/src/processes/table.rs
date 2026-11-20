use std::cell::RefCell;
use std::rc::Rc;

use rikka_app::prelude::*;

use crate::processes::row::{Row, Scale};
use crate::sampler::Reading;

pub(crate) struct Snapshot {
    pub(crate) reading: Rc<Reading>,
    pub(crate) rows: Vec<Row>,
    pub(crate) scale: Scale,
}

#[derive(Clone)]
pub(crate) struct RowTable(Rc<Inner>);

struct Inner {
    snapshot: RefCell<Rc<Snapshot>>,
    observers: Observers,
}

impl RowTable {
    pub(crate) fn new() -> RowTable {
        RowTable(Rc::new(Inner {
            snapshot: RefCell::new(Rc::new(Snapshot {
                reading: Rc::new(Reading::default()),
                rows: Vec::new(),
                scale: Scale::new(&[], 0),
            })),
            observers: Observers::new(),
        }))
    }

    pub(crate) fn get(&self) -> Rc<Snapshot> {
        self.0.observers.read();
        self.peek()
    }

    pub(crate) fn peek(&self) -> Rc<Snapshot> {
        Rc::clone(&self.0.snapshot.borrow())
    }

    pub(crate) fn set(&self, snapshot: Rc<Snapshot>) {
        *self.0.snapshot.borrow_mut() = snapshot;
        self.0.observers.changed();
    }
}

impl Observed for RowTable {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}

impl std::fmt::Debug for RowTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RowTable")
            .field("rows", &self.peek().rows.len())
            .finish()
    }
}
