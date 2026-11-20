#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Geometry {
    #[must_use]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub(crate) fn clamped(self) -> Self {
        let position = |value: i32| value.clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        let extent = |value: u32| value.clamp(1, u32::from(u16::MAX));
        Self {
            x: position(self.x),
            y: position(self.y),
            width: extent(self.width),
            height: extent(self.height),
        }
    }
}
