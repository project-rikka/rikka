use std::cell::RefCell;
use std::rc::Rc;

use crate::tracked::Tracked;

pub type UnmapCallback = Box<dyn FnOnce()>;

#[derive(Clone, Default)]
pub struct Presence {
    shown: Tracked<bool>,
    unmap_callbacks: Rc<RefCell<Vec<UnmapCallback>>>,
}

impl Presence {
    #[must_use]
    pub fn is_shown(&self) -> bool {
        self.shown.get()
    }

    pub fn set(&self, shown: bool) -> bool {
        self.shown.set(shown)
    }

    pub fn reopen(&self) {
        self.shown.renew(true);
    }

    #[must_use]
    pub fn shares(&self, other: &Presence) -> bool {
        Rc::ptr_eq(&self.unmap_callbacks, &other.unmap_callbacks)
    }

    pub fn take_moved(&self) -> bool {
        self.shown.take_moved()
    }

    pub fn after_unmap(&self, callback: impl FnOnce() + 'static) {
        self.unmap_callbacks.borrow_mut().push(Box::new(callback));
    }

    pub fn has_unmap_callbacks(&self) -> bool {
        !self.unmap_callbacks.borrow().is_empty()
    }

    pub fn take_unmap_callbacks(&self) -> Vec<UnmapCallback> {
        std::mem::take(&mut self.unmap_callbacks.borrow_mut())
    }
}

impl std::fmt::Debug for Presence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Presence")
            .field("shown", &self.is_shown())
            .field("unmap_callbacks", &self.unmap_callbacks.borrow().len())
            .finish()
    }
}
