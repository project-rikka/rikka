use crate::size::Size;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}

impl PixelSize {
    pub const ZERO: Self = Self {
        width: 0,
        height: 0,
    };

    #[inline]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    #[inline]
    pub fn aspect(self) -> f32 {
        self.width as f32 / self.height.max(1) as f32
    }

    #[inline]
    pub const fn area(self) -> u64 {
        self.width as u64 * self.height as u64
    }

    #[inline]
    pub fn to_size(self) -> Size {
        Size::new(self.width as f32, self.height as f32)
    }
}

impl From<(u32, u32)> for PixelSize {
    #[inline]
    fn from((width, height): (u32, u32)) -> Self {
        Self::new(width, height)
    }
}

impl From<PixelSize> for (u32, u32) {
    #[inline]
    fn from(size: PixelSize) -> Self {
        (size.width, size.height)
    }
}
