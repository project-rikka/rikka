use std::cell::Cell;
use std::rc::Rc;

use rikka_seam::SeamClient;
use rikka_types::Color;
use shell_kit::{Spot, Waiting};

use crate::editing::{Editing, Opening};
use crate::tools::{INKS, Pick, Width};

#[derive(Clone, Debug)]
pub(crate) struct Shared {
    pub(crate) seam: SeamClient,
    pub(crate) editing: Editing,
    pub(crate) waiting: Waiting<Opening>,
    pub(crate) choices: Choices,
    pub(crate) spot: Spot,
}

impl Shared {
    #[must_use]
    pub(crate) fn new(seam: SeamClient, editing: Editing, waiting: Waiting<Opening>) -> Shared {
        Shared {
            seam,
            editing,
            waiting,
            choices: Choices::default(),
            spot: Spot::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Picked {
    pub(crate) pick: Pick,
    pub(crate) ink: Color,
    pub(crate) width: Width,
}

impl Default for Picked {
    fn default() -> Picked {
        Picked {
            pick: Pick::default(),
            ink: INKS[0],
            width: Width::default(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Choices(Rc<Cell<Picked>>);

impl Choices {
    #[must_use]
    pub(crate) fn get(&self) -> Picked {
        self.0.get()
    }

    pub(crate) fn set(&self, picked: Picked) {
        self.0.set(picked);
    }
}
