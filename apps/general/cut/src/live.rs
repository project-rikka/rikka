use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use cut_project::{Appearance, ClipId, EffectSlot, Mix, Project};
use cut_render::Overrides;
use rikka_ui::{Observed, Observers};

#[derive(Clone, Default)]
pub(crate) struct LivePreview(Rc<Inner>);

#[derive(Default)]
struct Inner {
    overrides: RefCell<Overrides>,
    reworked: RefCell<Option<Arc<Project>>>,
    peek: Cell<Option<i64>>,
    observers: Observers,
}

impl LivePreview {
    pub(crate) fn same(&self, other: &LivePreview) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn effects(&self, clip: ClipId) -> Option<Vec<EffectSlot>> {
        self.0.observers.read();
        self.0
            .overrides
            .borrow()
            .effects(clip)
            .map(<[EffectSlot]>::to_vec)
    }

    pub(crate) fn appearance(&self, clip: ClipId) -> Option<Appearance> {
        self.0.observers.read();
        self.0.overrides.borrow().appearance(clip).cloned()
    }

    pub(crate) fn overrides(&self) -> Overrides {
        self.0.observers.read();
        self.0.overrides.borrow().clone()
    }

    pub(crate) fn project_or(&self, fallback: &Arc<Project>) -> Arc<Project> {
        self.0.observers.read();
        match &*self.0.reworked.borrow() {
            Some(reworked) => Arc::clone(reworked),
            None => Arc::clone(fallback),
        }
    }

    pub(crate) fn rework(&self, original: &Project, project: Project) {
        let unchanged = match &*self.0.reworked.borrow() {
            Some(reworked) => **reworked == project,
            None => *original == project,
        };
        if unchanged {
            return;
        }
        *self.0.reworked.borrow_mut() = Some(Arc::new(project));
        self.0.observers.changed();
    }

    pub(crate) fn mix(&self, clip: ClipId) -> Option<Mix> {
        self.0.observers.read();
        match self.0.overrides.borrow().appearance(clip)? {
            Appearance::Footage(treatment) => Some(treatment.mix),
            Appearance::Graphic(_) | Appearance::Region(_) | Appearance::Camera(_) => None,
        }
    }

    pub(crate) fn show(&self, clip: ClipId, appearance: Appearance) {
        if self.0.overrides.borrow_mut().show(clip, appearance) {
            self.0.observers.changed();
        }
    }

    pub(crate) fn show_effects(&self, clip: ClipId, effects: Vec<EffectSlot>) {
        if self.0.overrides.borrow_mut().show_effects(clip, effects) {
            self.0.observers.changed();
        }
    }

    pub(crate) fn peek(&self) -> Option<i64> {
        self.0.observers.read();
        self.0.peek.get()
    }

    pub(crate) fn show_frame(&self, frame: i64) {
        if self.0.peek.replace(Some(frame)) != Some(frame) {
            self.0.observers.changed();
        }
    }

    pub(crate) fn settle(&self) {
        let peeked = self.0.peek.take().is_some();
        if self.0.overrides.borrow().is_empty() && self.0.reworked.borrow().is_none() && !peeked {
            return;
        }
        self.0.overrides.borrow_mut().clear();
        self.0.reworked.borrow_mut().take();
        self.0.observers.changed();
    }
}

impl Observed for LivePreview {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}

impl std::fmt::Debug for LivePreview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Live")
            .field("shown", &self.0.overrides.borrow())
            .field("reworked", &self.0.reworked.borrow().is_some())
            .finish()
    }
}
