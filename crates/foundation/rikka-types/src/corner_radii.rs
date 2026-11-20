use crate::Size;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadii {
    pub const ZERO: Self = Self::uniform(0.0);

    #[inline]
    pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    #[inline]
    pub const fn uniform(radius: f32) -> Self {
        Self::new(radius, radius, radius, radius)
    }

    #[inline]
    pub fn max(self) -> f32 {
        self.top_left
            .max(self.top_right)
            .max(self.bottom_right)
            .max(self.bottom_left)
    }

    pub fn clamp_for_size(self, size: Size) -> Self {
        let limit = (size.min_side() * 0.5).max(0.0);
        Self::new(
            self.top_left.clamp(0.0, limit),
            self.top_right.clamp(0.0, limit),
            self.bottom_right.clamp(0.0, limit),
            self.bottom_left.clamp(0.0, limit),
        )
    }

    #[inline]
    pub fn to_array(self) -> [f32; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }
}

impl From<f32> for CornerRadii {
    #[inline]
    fn from(radius: f32) -> Self {
        Self::uniform(radius)
    }
}

impl std::ops::Mul<f32> for CornerRadii {
    type Output = Self;

    #[inline]
    fn mul(self, factor: f32) -> Self {
        Self::new(
            self.top_left * factor,
            self.top_right * factor,
            self.bottom_right * factor,
            self.bottom_left * factor,
        )
    }
}
