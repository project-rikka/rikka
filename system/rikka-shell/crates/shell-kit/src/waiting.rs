use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug)]
pub struct Waiting<T>(Rc<RefCell<Option<T>>>);

impl<T> Waiting<T> {
    pub(crate) fn deliver(&self, waiting: T) {
        *self.0.borrow_mut() = Some(waiting);
    }

    #[must_use]
    pub fn is_waiting(&self) -> bool {
        self.0.borrow().is_some()
    }

    #[must_use]
    pub fn take(&self) -> Option<T> {
        self.0.borrow_mut().take()
    }
}

impl<T> Clone for Waiting<T> {
    fn clone(&self) -> Waiting<T> {
        Waiting(Rc::clone(&self.0))
    }
}

impl<T> Default for Waiting<T> {
    fn default() -> Waiting<T> {
        Waiting(Rc::default())
    }
}
