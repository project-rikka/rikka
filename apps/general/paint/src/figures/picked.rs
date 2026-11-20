use std::cell::RefCell;
use std::rc::Rc;

use paint_document::{Figure, FigureId};
use rikka_ui::{Observed, Observers};

#[derive(Clone, Default)]
pub(crate) struct FigureSelection(Rc<Inner>);

#[derive(Default)]
struct Inner {
    ids: RefCell<Vec<FigureId>>,
    live: RefCell<Option<Vec<Figure>>>,
    settling: RefCell<Option<(Vec<Figure>, u64)>>,
    observers: Observers,
}

impl FigureSelection {
    pub(crate) fn same(&self, other: &FigureSelection) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn ids(&self) -> Vec<FigureId> {
        self.0.observers.read();
        self.0.ids.borrow().clone()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.observers.read();
        self.0.ids.borrow().is_empty()
    }

    pub(crate) fn holds(&self, id: FigureId) -> bool {
        self.0.ids.borrow().contains(&id)
    }

    pub(crate) fn set(&self, ids: Vec<FigureId>) {
        let changed = *self.0.ids.borrow() != ids;
        if changed {
            *self.0.ids.borrow_mut() = ids;
            *self.0.live.borrow_mut() = None;
            self.0.observers.changed();
        }
    }

    pub(crate) fn keep(&self, present: impl Fn(FigureId) -> bool) {
        let before = self.0.ids.borrow().len();
        self.0.ids.borrow_mut().retain(|id| present(*id));
        if self.0.ids.borrow().len() != before {
            self.0.observers.changed();
        }
    }

    pub(crate) fn show_live(&self, figures: Option<Vec<Figure>>) {
        *self.0.live.borrow_mut() = figures;
        self.0.observers.changed();
    }

    pub(crate) fn settle(&self, figures: Vec<Figure>, generation: u64) {
        *self.0.live.borrow_mut() = None;
        *self.0.settling.borrow_mut() = Some((figures, generation));
        self.0.observers.changed();
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
            self.0.observers.changed();
        }
    }

    pub(crate) fn with_preview(&self, committed: &[Figure]) -> Vec<Figure> {
        self.0.observers.read();
        if let Some(live) = self.0.live.borrow().as_ref() {
            return live.clone();
        }
        let ids = self.0.ids.borrow();
        let settling = self.0.settling.borrow();
        let source = settling
            .as_ref()
            .map_or(committed, |(figures, _)| figures.as_slice());
        source
            .iter()
            .filter(|figure| ids.contains(&figure.id))
            .cloned()
            .collect()
    }
}

impl Observed for FigureSelection {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}

impl std::fmt::Debug for FigureSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Picked")
            .field("ids", &self.0.ids.borrow())
            .field("live", &self.0.live.borrow().is_some())
            .finish_non_exhaustive()
    }
}
