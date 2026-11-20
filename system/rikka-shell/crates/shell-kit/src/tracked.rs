use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct Tracked<T> {
    value: Rc<RefCell<T>>,
    version: Rc<Cell<u64>>,
    seen: Cell<u64>,
}

impl<T: Clone + PartialEq> Tracked<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value: Rc::new(RefCell::new(value)),
            version: Rc::default(),
            seen: Cell::new(0),
        }
    }

    pub fn get(&self) -> T {
        self.value.borrow().clone()
    }

    pub fn set(&self, value: T) -> bool {
        if *self.value.borrow() == value {
            return false;
        }
        *self.value.borrow_mut() = value;
        self.version.set(self.version.get() + 1);
        true
    }

    pub(crate) fn renew(&self, value: T) {
        *self.value.borrow_mut() = value;
        self.version.set(self.version.get() + 1);
    }

    pub(super) fn take_moved(&self) -> bool {
        let version = self.version.get();
        self.seen.replace(version) != version
    }
}

impl<T> Clone for Tracked<T> {
    fn clone(&self) -> Self {
        Self {
            value: Rc::clone(&self.value),
            version: Rc::clone(&self.version),
            seen: Cell::new(self.seen.get()),
        }
    }
}

impl<T: Default> Default for Tracked<T> {
    fn default() -> Self {
        Self {
            value: Rc::default(),
            version: Rc::default(),
            seen: Cell::new(0),
        }
    }
}

impl<T: Clone + PartialEq + std::fmt::Debug> std::fmt::Debug for Tracked<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Tracked").field(&self.get()).finish()
    }
}
