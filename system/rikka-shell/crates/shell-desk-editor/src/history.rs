use std::cell::{Cell, RefCell};
use std::rc::Rc;

use desk_layout::{Layout, Undo, WidgetId};
use rikka_ui::{Observed, Observers};

pub(crate) type Step = (Option<String>, Layout);

#[derive(Clone, Default)]
pub(crate) struct History(Rc<Inner>);

#[derive(Default)]
struct Inner {
    undo: RefCell<Undo<Step>>,
    nudging: Cell<Option<WidgetId>>,
    observers: Observers,
}

impl History {
    pub(crate) fn record(&self, before: Step) {
        self.0.nudging.set(None);
        self.0.undo.borrow_mut().record(before);
        self.0.observers.changed();
    }

    pub(super) fn record_nudge(&self, widget: WidgetId, before: Step) {
        if self.0.nudging.replace(Some(widget)) == Some(widget) {
            return;
        }
        self.0.undo.borrow_mut().record(before);
        self.0.observers.changed();
    }

    pub(crate) fn undo(&self, now: impl FnOnce(Option<&str>) -> Layout) -> Option<Step> {
        self.walk(now, |undo| undo.earlier(), Undo::undo)
    }

    pub(crate) fn redo(&self, now: impl FnOnce(Option<&str>) -> Layout) -> Option<Step> {
        self.walk(now, |undo| undo.later(), Undo::redo)
    }

    #[must_use]
    pub(crate) fn can_undo(&self) -> bool {
        self.0.observers.read();
        self.0.undo.borrow().can_undo()
    }

    #[must_use]
    pub(crate) fn can_redo(&self) -> bool {
        self.0.observers.read();
        self.0.undo.borrow().can_redo()
    }

    pub(crate) fn forget(&self) {
        self.0.nudging.set(None);
        self.0.undo.borrow_mut().forget();
        self.0.observers.changed();
    }

    fn walk(
        &self,
        now: impl FnOnce(Option<&str>) -> Layout,
        next: impl FnOnce(&Undo<Step>) -> Option<&Step>,
        step: impl FnOnce(&mut Undo<Step>, Step) -> Option<Step>,
    ) -> Option<Step> {
        self.0.nudging.set(None);
        let wallpaper = next(&self.0.undo.borrow())?.0.clone();
        let current = now(wallpaper.as_deref());
        let applied_step = step(&mut self.0.undo.borrow_mut(), (wallpaper, current));
        self.0.observers.changed();
        applied_step
    }
}

impl Observed for History {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}

impl std::fmt::Debug for History {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("History").finish_non_exhaustive()
    }
}
