use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub(crate) struct Chosen(Rc<RefCell<String>>);

impl Chosen {
    pub(crate) fn set(&self, value: String) {
        *self.0.borrow_mut() = value;
    }

    pub(crate) fn is(&self, value: &str) -> bool {
        *self.0.borrow() == value
    }
}
