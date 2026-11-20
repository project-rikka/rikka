use crate::input::SCROLL_LINE;
use crate::point::Point;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollDelta {
    offset: Point,
    natural: bool,
}

impl ScrollDelta {
    pub const ZERO: Self = Self::from_offset(Point::ZERO);

    #[must_use]
    pub const fn from_offset(offset: Point) -> Self {
        Self {
            offset,
            natural: false,
        }
    }

    #[must_use]
    pub fn from_content(content: Point) -> Self {
        Self::from_offset(-content)
    }

    #[must_use]
    pub fn from_up(notches: f32) -> Self {
        Self::from_offset(Point::new(0.0, -notches * SCROLL_LINE))
    }

    #[must_use]
    pub const fn with_natural(mut self, natural: bool) -> Self {
        self.natural = natural;
        self
    }

    #[must_use]
    pub const fn offset(self) -> Point {
        self.offset
    }

    #[must_use]
    pub fn content(self) -> Point {
        -self.offset
    }

    #[must_use]
    pub const fn is_natural(self) -> bool {
        self.natural
    }

    #[must_use]
    pub fn up(self) -> f32 {
        -self.physical().y / SCROLL_LINE
    }

    #[must_use]
    pub fn right(self) -> f32 {
        self.physical().x / SCROLL_LINE
    }

    #[must_use]
    pub fn zoom(self) -> f32 {
        let (up, right) = (self.up(), self.right());
        match up.abs() >= right.abs() {
            true => up,
            false => right,
        }
    }

    #[must_use]
    pub fn scaled(mut self, factor: f32) -> Self {
        self.offset = self.offset * factor;
        self
    }

    fn physical(self) -> Point {
        match self.natural {
            true => -self.offset,
            false => self.offset,
        }
    }
}
