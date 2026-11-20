use rikka_input::xkb;
use rikka_types::Modifiers;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModifierState {
    pub depressed: Modifiers,
    pub caps_lock: bool,
}

impl ModifierState {
    pub(crate) fn depressed_mask(self) -> u32 {
        xkb::mask(self.depressed)
    }

    pub(crate) fn locked_mask(self) -> u32 {
        match self.caps_lock {
            true => xkb::LOCK,
            false => 0,
        }
    }
}

impl From<Modifiers> for ModifierState {
    fn from(depressed: Modifiers) -> ModifierState {
        ModifierState {
            depressed,
            caps_lock: false,
        }
    }
}
