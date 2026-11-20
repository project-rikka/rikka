#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Bounds {
    pub(crate) const fn right(self) -> i32 {
        self.x + self.width
    }

    pub(crate) const fn bottom(self) -> i32 {
        self.y + self.height
    }

    pub(crate) const fn overlaps(self, other: Self) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
}
