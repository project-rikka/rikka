use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::win32::runner::Runner;

thread_local! {
    static ACTIVE: RefCell<Weak<RefCell<Runner>>> = const { RefCell::new(Weak::new()) };
}

pub(crate) struct Installed;

pub(crate) fn install(runner: &Rc<RefCell<Runner>>) -> Installed {
    ACTIVE.with_borrow_mut(|active| *active = Rc::downgrade(runner));
    Installed
}

impl Drop for Installed {
    fn drop(&mut self) {
        ACTIVE.with_borrow_mut(|active| *active = Weak::new());
    }
}

pub(crate) fn with_runner<T>(act: impl FnOnce(&mut Runner) -> T) -> Option<T> {
    let runner = ACTIVE.with_borrow(Weak::upgrade)?;
    let mut borrowed = runner.try_borrow_mut().ok()?;
    Some(act(&mut borrowed))
}
