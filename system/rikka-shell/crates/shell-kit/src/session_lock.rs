use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rikka_ui::Observers;

use crate::Presence;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sealed {
    #[default]
    Open,
    Locking,
    Locked,
}

type LockCallback = Rc<dyn Fn(Sealed)>;

#[derive(Clone, Default)]
pub struct SessionLock(Rc<Inner>);

#[derive(Default)]
struct Inner {
    presence: Presence,
    sealed: Cell<Sealed>,
    observers: Observers,
    event: RefCell<Vec<LockCallback>>,
}

impl SessionLock {
    pub fn lock(&self) {
        self.0.presence.set(true);
    }

    pub fn unlock(&self) {
        self.0.presence.set(false);
    }

    #[must_use]
    pub fn is_wanted(&self) -> bool {
        self.0.presence.is_shown()
    }

    #[must_use]
    pub fn sealed(&self) -> Sealed {
        self.0.observers.read();
        self.0.sealed.get()
    }

    pub fn on_change(&self, event: impl Fn(Sealed) + 'static) {
        self.0.event.borrow_mut().push(Rc::new(event));
    }

    pub fn presence(&self) -> Presence {
        self.0.presence.clone()
    }

    pub fn settle(&self, sealed: Sealed) {
        if self.0.sealed.replace(sealed) == sealed {
            return;
        }
        self.0.observers.changed();
        let event: Vec<LockCallback> = self.0.event.borrow().clone();
        for event in event {
            event(sealed);
        }
    }
}

impl std::fmt::Debug for SessionLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionLock")
            .field("wanted", &self.is_wanted())
            .field("sealed", &self.0.sealed.get())
            .finish_non_exhaustive()
    }
}
