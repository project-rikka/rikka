use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    #[must_use]
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Region {
        Region {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub const fn from_size(width: u32, height: u32) -> Region {
        Region::new(0, 0, width, height)
    }

    #[must_use]
    pub fn centered(outer: (u32, u32), width: u32, height: u32) -> Region {
        Region::new(
            outer.0.saturating_sub(width) / 2,
            outer.1.saturating_sub(height) / 2,
            width,
            height,
        )
    }

    #[must_use]
    pub const fn right(self) -> u32 {
        self.x.saturating_add(self.width)
    }

    #[must_use]
    pub const fn bottom(self) -> u32 {
        self.y.saturating_add(self.height)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    #[must_use]
    pub const fn area(self) -> u64 {
        self.width as u64 * self.height as u64
    }

    #[must_use]
    pub const fn contains(self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    #[must_use]
    pub fn clamp_to(self, width: u32, height: u32) -> Option<Region> {
        let x = self.x.min(width);
        let y = self.y.min(height);
        let clamped = Region::new(
            x,
            y,
            self.right().min(width).saturating_sub(x),
            self.bottom().min(height).saturating_sub(y),
        );
        (!clamped.is_empty()).then_some(clamped)
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}+{}+{}", self.width, self.height, self.x, self.y)
    }
}
