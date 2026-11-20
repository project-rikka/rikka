use std::cell::Cell;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    #[default]
    Picture,
    Video,
    Translate,
    Scan,
}

impl Mode {
    pub(crate) const ALL: [Mode; 4] = [Mode::Picture, Mode::Video, Mode::Translate, Mode::Scan];
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Target {
    #[default]
    Region,
    Window,
    Screen,
}

impl Target {
    pub(crate) const ALL: [Target; 3] = [Target::Region, Target::Window, Target::Screen];
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct Aim {
    pub(crate) mode: Mode,
    pub(crate) target: Target,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Aimed(Rc<Cell<Aim>>);

impl Aimed {
    #[must_use]
    pub(crate) fn get(&self) -> Aim {
        self.0.get()
    }

    pub(crate) fn set(&self, aim: Aim) {
        self.0.set(aim);
    }

    #[must_use]
    pub(crate) fn mode(&self) -> Mode {
        self.0.get().mode
    }

    #[must_use]
    pub(crate) fn target(&self) -> Target {
        self.0.get().target
    }

    #[must_use]
    pub(super) fn framing(&self) -> Target {
        match self.mode() {
            Mode::Scan => Target::Region,
            _ => self.target(),
        }
    }
}
