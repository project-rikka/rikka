use rikka_physics_math::Vec3;

pub const MAX_PROXY_POINTS: usize = 255;

#[derive(Clone, Copy, Debug)]
pub struct ShapeProxy<'a> {
    pub points: &'a [Vec3],
    pub radius: f32,
}

impl<'a> ShapeProxy<'a> {
    #[inline]
    pub fn new(points: &'a [Vec3], radius: f32) -> Self {
        debug_assert!(!points.is_empty() && points.len() <= MAX_PROXY_POINTS);
        Self { points, radius }
    }

    #[inline]
    pub fn support(&self, direction: Vec3) -> usize {
        let mut best = 0;
        let mut best_value = self.points[0].dot(direction);
        for (index, point) in self.points.iter().enumerate().skip(1) {
            let value = point.dot(direction);
            if value > best_value {
                best = index;
                best_value = value;
            }
        }
        best
    }
}
