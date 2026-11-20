use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub(crate) struct Grip(Rc<GripState>);

#[derive(Debug, Default)]
struct GripState {
    space: Cell<bool>,
    panning: Cell<bool>,
}

impl Grip {
    pub(crate) fn hold_space(&self, pressed: bool) {
        self.0.space.set(pressed);
    }

    pub(crate) fn space_held(&self) -> bool {
        self.0.space.get()
    }

    pub(crate) fn set_panning(&self, panning: bool) {
        self.0.panning.set(panning);
    }

    pub(crate) fn is_panning(&self) -> bool {
        self.0.panning.get()
    }

    pub(crate) fn same(&self, other: &Grip) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
