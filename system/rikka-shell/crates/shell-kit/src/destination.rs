use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub struct Destination(Rc<RefCell<Option<String>>>);

impl Destination {
    pub fn aim(&self, output: impl Into<String>) {
        *self.0.borrow_mut() = Some(output.into());
    }

    #[must_use]
    pub fn output(&self) -> Option<String> {
        self.0.borrow().clone()
    }
}
