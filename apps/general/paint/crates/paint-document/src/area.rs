#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Area {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

impl Area {
    #[must_use]
    pub const fn new(left: u32, top: u32, width: u32, height: u32) -> Area {
        Area {
            left,
            top,
            width,
            height,
        }
    }

    #[must_use]
    pub const fn sized(width: u32, height: u32) -> Area {
        Area::new(0, 0, width, height)
    }

    #[must_use]
    pub fn between(corner: (u32, u32), opposite: (u32, u32)) -> Area {
        let (left, right) = (corner.0.min(opposite.0), corner.0.max(opposite.0));
        let (top, bottom) = (corner.1.min(opposite.1), corner.1.max(opposite.1));
        Area::new(left, top, right - left, bottom - top)
    }

    #[must_use]
    pub fn right(self) -> u32 {
        self.left + self.width
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}
