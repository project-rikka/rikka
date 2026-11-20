use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Plane {
    pub normal: Vec3,
    pub offset: f32,
}

impl Plane {
    #[inline]
    pub const fn new(normal: Vec3, offset: f32) -> Self {
        Self { normal, offset }
    }

    #[inline]
    pub const fn up() -> Self {
        Self::new(Vec3::Y, 0.0)
    }

    #[inline]
    pub fn from_normal_point(normal: Vec3, point: Vec3) -> Self {
        Self::new(normal, normal.dot(point))
    }

    #[inline]
    pub fn separation(&self, point: Vec3) -> f32 {
        self.normal.dot(point) - self.offset
    }

    #[inline]
    pub fn project(&self, point: Vec3) -> Vec3 {
        point - self.separation(point) * self.normal
    }
}
