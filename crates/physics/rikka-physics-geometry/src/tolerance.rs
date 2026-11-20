use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerance {
    pub length_scale: f32,
}

impl Default for Tolerance {
    #[inline(always)]
    fn default() -> Self {
        Self { length_scale: 1.0 }
    }
}

impl Tolerance {
    pub const MAX_ROTATION: f32 = 0.25 * PI;

    #[inline(always)]
    pub fn linear_slop(&self) -> f32 {
        0.005 * self.length_scale
    }

    #[inline(always)]
    pub fn speculative_distance(&self) -> f32 {
        4.0 * self.linear_slop()
    }

    #[inline]
    pub fn aabb_margin(&self, size: f32) -> f32 {
        (0.05 * self.length_scale).min(0.125 * size)
    }
}
