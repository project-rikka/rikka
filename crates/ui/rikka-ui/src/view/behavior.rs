use std::ops::{BitOr, BitOrAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Behavior(u16);

impl Behavior {
    pub const NONE: Self = Self(0);

    pub const CLIPS: Self = Self(1 << 0);

    pub const OVERLAY: Self = Self(1 << 1);

    pub const PAINTS_OFFSCREEN: Self = Self(1 << 2);

    pub const MODAL: Self = Self(1 << 3);

    pub const TRACKS_HOVER: Self = Self(1 << 4);

    pub const FOCUSABLE: Self = Self(1 << 5);

    pub const NO_TAB_STOP: Self = Self(1 << 6);

    pub const AUTOFOCUS: Self = Self(1 << 7);

    pub const TAKES_FOCUS: Self = Self(1 << 8);

    pub const TAKES_TAB: Self = Self(1 << 9);

    pub const GRABS_KEYS: Self = Self(1 << 10);

    pub const INERT: Self = Self(1 << 11);

    pub const CATCHES_DROPS: Self = Self(1 << 12);

    pub const TRACKS_PLACE: Self = Self(1 << 13);

    pub const TRAVELS: Self = Self(1 << 14);

    pub const INTERCEPTS: Self = Self(1 << 15);

    #[must_use]
    pub const fn has(self, behavior: Behavior) -> bool {
        self.0 & behavior.0 == behavior.0
    }

    #[must_use]
    pub const fn when(self, yes: bool) -> Self {
        match yes {
            true => self,
            false => Self::NONE,
        }
    }

    #[must_use]
    pub const fn focusable(self) -> bool {
        self.has(Self::FOCUSABLE)
    }

    #[must_use]
    pub const fn tab_stop(self) -> bool {
        self.has(Self::FOCUSABLE) && !self.has(Self::NO_TAB_STOP)
    }
}

impl BitOr for Behavior {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl BitOrAssign for Behavior {
    fn bitor_assign(&mut self, other: Self) {
        self.0 |= other.0;
    }
}
