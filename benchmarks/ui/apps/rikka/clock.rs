use std::cell::Cell;
use std::rc::Rc;

use rikka_ui::{Observed, Observers};

#[derive(Clone, Debug, Default)]
pub(crate) struct Clock(Rc<ClockState>);

#[derive(Debug, Default)]
struct ClockState {
    frame: Cell<u64>,
    observers: Observers,
}

impl Clock {
    pub(crate) fn frame(&self) -> u64 {
        self.0.observers.read();
        self.0.frame.get()
    }

    pub(crate) fn tick(&self) {
        self.0.frame.set(self.0.frame.get() + 1);
        self.0.observers.changed();
    }
}

impl Observed for Clock {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}
