use std::cell::Cell;
use std::rc::Rc;

use rikka_ui::{Observed, Observers};

#[derive(Clone, Debug, Default)]
pub(crate) struct Pointer(Rc<Inner>);

#[derive(Debug, Default)]
struct Inner {
    at: Cell<Option<(u32, u32)>>,
    observers: Observers,
}

impl Pointer {
    pub(crate) fn at(&self) -> Option<(u32, u32)> {
        self.0.observers.read();
        self.0.at.get()
    }

    pub(crate) fn set(&self, at: Option<(u32, u32)>) {
        if self.0.at.replace(at) != at {
            self.0.observers.changed();
        }
    }

    pub(crate) fn same(&self, other: &Pointer) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Observed for Pointer {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}
