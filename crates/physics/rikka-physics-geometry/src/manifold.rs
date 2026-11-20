use rikka_physics_math::Vec3;

pub const MAX_MANIFOLD_POINTS: usize = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ManifoldPoint {
    pub point: Vec3,
    pub separation: f32,
    pub feature: u32,
    pub triangle: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Manifold {
    pub normal: Vec3,
    pub points: [ManifoldPoint; MAX_MANIFOLD_POINTS],
    pub count: u32,
    pub material: u32,
}

impl Manifold {
    #[inline]
    pub fn points(&self) -> &[ManifoldPoint] {
        &self.points[..self.count as usize]
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    #[inline(always)]
    pub fn push(&mut self, point: ManifoldPoint) {
        self.points[self.count as usize] = point;
        self.count += 1;
    }
}
