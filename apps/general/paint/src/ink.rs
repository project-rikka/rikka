use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rikka_ui::{Observed, Observers};

use crate::stroke::{Measure, Stroke};

#[derive(Clone, Debug, Default)]
pub(crate) struct Ink(Rc<Inner>);

#[derive(Debug, Default)]
struct Inner {
    live: RefCell<Option<Stroke>>,
    settling: RefCell<Option<(Stroke, u64)>>,
    revision: Cell<u64>,
    observers: Observers,
}

impl Ink {
    pub(crate) fn same(&self, other: &Ink) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn begin(&self, stroke: Stroke) {
        *self.0.live.borrow_mut() = Some(stroke);
        self.touched();
    }

    pub(crate) fn change(&self, change: impl FnOnce(&mut Stroke) -> bool) {
        let changed = self.0.live.borrow_mut().as_mut().is_some_and(change);
        if changed {
            self.touched();
        }
    }

    pub(crate) fn take(&self) -> Option<Stroke> {
        let stroke = self.0.live.borrow_mut().take();
        if stroke.is_some() {
            self.touched();
        }
        stroke
    }

    pub(crate) fn settle(&self, stroke: Stroke, generation: u64) {
        *self.0.settling.borrow_mut() = Some((stroke, generation));
        self.touched();
    }

    pub(crate) fn settled(&self, generation: u64) {
        let done = self
            .0
            .settling
            .borrow()
            .as_ref()
            .is_some_and(|(_, settles)| *settles <= generation);
        if done {
            *self.0.settling.borrow_mut() = None;
            self.touched();
        }
    }

    pub(crate) fn revision(&self) -> u64 {
        self.0.observers.read();
        self.0.revision.get()
    }

    pub(crate) fn with_stroke<R>(&self, read: impl FnOnce(Option<&Stroke>) -> R) -> R {
        self.0.observers.read();
        let live = self.0.live.borrow();
        let settling = self.0.settling.borrow();
        read(
            live.as_ref()
                .or_else(|| settling.as_ref().map(|(stroke, _)| stroke)),
        )
    }

    pub(crate) fn measure(&self) -> Option<Measure> {
        self.0.observers.read();
        self.0.live.borrow().as_ref().and_then(Stroke::measure)
    }

    fn touched(&self) {
        self.0.revision.set(self.0.revision.get() + 1);
        self.0.observers.changed();
    }
}

impl Observed for Ink {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}
