#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    LeftToRight,
    RightToLeft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BaseDirection {
    Auto,
    LeftToRight,
    RightToLeft,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Level(u8);

impl Level {
    pub const LEFT_TO_RIGHT: Self = Self(0);
    pub const RIGHT_TO_LEFT: Self = Self(1);
    pub const MAX_EXPLICIT: u8 = 125;
    pub const MAX: u8 = Self::MAX_EXPLICIT + 1;

    pub const fn new(number: u8) -> Option<Self> {
        if number <= Self::MAX {
            Some(Self(number))
        } else {
            None
        }
    }

    pub(crate) const fn from_resolved(number: u8) -> Self {
        Self(number)
    }

    #[inline]
    pub const fn number(self) -> u8 {
        self.0
    }

    #[inline]
    pub const fn is_rtl(self) -> bool {
        self.0 & 1 == 1
    }

    #[inline]
    pub const fn direction(self) -> Direction {
        if self.is_rtl() {
            Direction::RightToLeft
        } else {
            Direction::LeftToRight
        }
    }
}
