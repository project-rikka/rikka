use rikka_types::Modifiers;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Holding(Modifiers);

impl Holding {
    pub fn of(pressed: Modifiers) -> Self {
        Self(Modifiers {
            shift: false,
            ..pressed
        })
    }

    pub fn is_sticky(self) -> bool {
        self.0 == Modifiers::NONE
    }

    pub fn held_by(self, pressed: Modifiers) -> bool {
        (self.0.control && pressed.control)
            || (self.0.alt && pressed.alt)
            || (self.0.logo && pressed.logo)
    }
}
