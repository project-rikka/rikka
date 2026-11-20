use std::cell::Cell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::view::{Observed, Observers};

#[derive(Clone, Debug, Default)]
pub struct Place(Rc<PlaceState>);

#[derive(Debug, Default)]
struct PlaceState {
    stand: Cell<ScrollPosition>,
    ending: Cell<bool>,
    observers: Observers,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollPosition {
    pub subject: Option<u64>,
    pub offset: f32,
    pub ended: bool,
    pub away: bool,
    pub row: Option<(usize, f32)>,
}

impl Place {
    #[must_use]
    pub fn new() -> Place {
        Place::default()
    }

    pub fn top(&self) {
        self.set(ScrollPosition::default());
    }

    pub fn end(&self) {
        self.0.ending.set(true);
        self.0.observers.changed();
    }

    pub fn get(&self) -> ScrollPosition {
        self.0.observers.read();
        self.0.stand.get()
    }

    pub fn is_scrolled_away(&self) -> bool {
        self.get().away
    }

    pub fn set(&self, stand: ScrollPosition) {
        if self.0.stand.replace(stand) != stand {
            self.0.observers.changed();
        }
    }

    pub(crate) fn record(&self, stand: ScrollPosition) {
        if self.0.stand.replace(stand).away != stand.away {
            self.0.observers.changed();
        }
    }

    pub(crate) fn take_end(&self) -> bool {
        self.0.ending.replace(false)
    }
}

impl Hash for Place {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.0).hash(state);
    }
}

impl Observed for Place {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}
