use core::ops::Mul;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    #[inline]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    #[inline]
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    #[inline]
    pub fn min_side(self) -> f32 {
        self.width.min(self.height)
    }

    #[inline]
    pub fn area(self) -> f32 {
        self.width * self.height
    }
}

impl Mul<f32> for Size {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.width * rhs, self.height * rhs)
    }
}

impl From<(f32, f32)> for Size {
    #[inline]
    fn from((width, height): (f32, f32)) -> Self {
        Self::new(width, height)
    }
}
