use rikka_types::Point;

use crate::rounding::ceil_to_i32;

const MAX_STEPS: f32 = 256.0;

#[derive(Clone, Copy, Debug)]
pub struct Flatness {
    quad: f32,
    cubic: f32,
}

impl Flatness {
    #[inline(always)]
    pub fn new(tolerance: f32) -> Self {
        Self {
            quad: 1.0 / (4.0 * tolerance),
            cubic: 0.75 / tolerance,
        }
    }

    #[inline(always)]
    pub fn quad_steps(self, p0: Point, p1: Point, p2: Point) -> i32 {
        let ddx = p0.x - 2.0 * p1.x + p2.x;
        let ddy = p0.y - 2.0 * p1.y + p2.y;
        let deviation = (ddx * ddx + ddy * ddy).sqrt();
        steps(deviation * self.quad)
    }

    #[inline(always)]
    pub fn cubic_steps(self, p0: Point, p1: Point, p2: Point, p3: Point) -> i32 {
        let d0 = (p0.x - 2.0 * p1.x + p2.x, p0.y - 2.0 * p1.y + p2.y);
        let d1 = (p1.x - 2.0 * p2.x + p3.x, p1.y - 2.0 * p2.y + p3.y);
        let deviation = (d0.0 * d0.0 + d0.1 * d0.1)
            .max(d1.0 * d1.0 + d1.1 * d1.1)
            .sqrt();
        steps(deviation * self.cubic)
    }
}

#[inline(always)]
fn steps(scaled_deviation: f32) -> i32 {
    ceil_to_i32(scaled_deviation.sqrt().min(MAX_STEPS)).max(1)
}
