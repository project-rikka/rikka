use rikka_physics_math::Vec3;

use crate::ShapeProxy;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayInput {
    pub origin: Vec3,
    pub translation: Vec3,
    pub max_fraction: f32,
}

impl RayInput {
    #[inline]
    pub fn new(origin: Vec3, translation: Vec3) -> Self {
        Self {
            origin,
            translation,
            max_fraction: 1.0,
        }
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.origin.is_finite()
            && self.translation.is_finite()
            && self.max_fraction.is_finite()
            && (0.0..=1.0e9).contains(&self.max_fraction)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ShapeCastInput<'a> {
    pub proxy: ShapeProxy<'a>,
    pub translation: Vec3,
    pub max_fraction: f32,
    pub can_encroach: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CastHit {
    pub point: Vec3,
    pub normal: Vec3,
    pub fraction: f32,
    pub triangle: u32,
    pub child: u32,
    pub material: u32,
}

impl CastHit {
    pub const NULL_INDEX: u32 = u32::MAX;

    #[inline]
    pub fn new(point: Vec3, normal: Vec3, fraction: f32) -> Self {
        Self {
            point,
            normal,
            fraction,
            triangle: Self::NULL_INDEX,
            child: Self::NULL_INDEX,
            material: 0,
        }
    }
}
