use rikka_app::prelude::{Color, PointerButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    Primary,
    Secondary,
}

impl Slot {
    pub(crate) fn from_button(button: PointerButton) -> Option<Slot> {
        match button {
            PointerButton::Primary => Some(Slot::Primary),
            PointerButton::Secondary => Some(Slot::Secondary),
            _ => None,
        }
    }

    pub(crate) fn other(self) -> Slot {
        match self {
            Slot::Primary => Slot::Secondary,
            Slot::Secondary => Slot::Primary,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Colors {
    pub(crate) primary: Color,
    pub(crate) secondary: Color,
}

impl Default for Colors {
    fn default() -> Colors {
        Colors {
            primary: Color::BLACK,
            secondary: Color::WHITE,
        }
    }
}

impl Colors {
    pub(crate) fn get(self, slot: Slot) -> Color {
        match slot {
            Slot::Primary => self.primary,
            Slot::Secondary => self.secondary,
        }
    }

    pub(crate) fn set(&mut self, slot: Slot, color: Color) {
        match slot {
            Slot::Primary => self.primary = color,
            Slot::Secondary => self.secondary = color,
        }
    }

    pub(crate) fn swap(&mut self) {
        std::mem::swap(&mut self.primary, &mut self.secondary);
    }

    pub(crate) fn reset(&mut self) {
        *self = Colors::default();
    }
}
