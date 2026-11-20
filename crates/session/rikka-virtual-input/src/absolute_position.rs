#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbsolutePosition {
    pub x: u32,
    pub y: u32,
    pub x_extent: u32,
    pub y_extent: u32,
}

impl AbsolutePosition {
    #[must_use]
    pub fn new(x: u32, y: u32, x_extent: u32, y_extent: u32) -> AbsolutePosition {
        AbsolutePosition {
            x,
            y,
            x_extent,
            y_extent,
        }
    }

    #[must_use]
    pub fn is_in_bounds(self) -> bool {
        self.x < self.x_extent && self.y < self.y_extent
    }
}
