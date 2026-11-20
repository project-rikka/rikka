use rikka_physics_geometry::{MAX_MANIFOLD_POINTS, Manifold};
use rikka_physics_math::Vec3;

use crate::MixedMaterial;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ContactPoint {
    pub anchor_a: Vec3,
    pub anchor_b: Vec3,
    pub point: Vec3,
    pub separation: f32,
    pub base_separation: f32,
    pub normal_impulse: f32,
    pub total_normal_impulse: f32,
    pub normal_velocity: f32,
    pub feature: u32,
    pub triangle: u32,
    pub persisted: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ContactManifold {
    pub normal: Vec3,
    pub count: u32,
    pub points: [ContactPoint; MAX_MANIFOLD_POINTS],
    pub material: MixedMaterial,
    pub friction_impulse: Vec3,
    pub twist_impulse: f32,
    pub rolling_impulse: Vec3,
}

impl ContactManifold {
    #[inline]
    pub fn points(&self) -> &[ContactPoint] {
        &self.points[..self.count as usize]
    }

    #[inline]
    pub fn points_mut(&mut self) -> &mut [ContactPoint] {
        &mut self.points[..self.count as usize]
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn max_normal_impulse(&self) -> f32 {
        self.points()
            .iter()
            .fold(0.0, |max, point| max.max(point.total_normal_impulse))
    }

    pub(crate) fn from_world(
        world: &Manifold,
        center_a: Vec3,
        center_b: Vec3,
        material: MixedMaterial,
        previous: &[ContactManifold],
    ) -> Self {
        let matches = |old: &ContactManifold| {
            world
                .points()
                .iter()
                .filter(|point| old.points().iter().any(|o| o.feature == point.feature))
                .count()
        };
        let best = previous
            .iter()
            .filter(|old| {
                old.material.user_a == material.user_a && old.material.user_b == material.user_b
            })
            .max_by_key(|old| matches(old))
            .map(Carry::from_manifold);
        let mut manifold = ContactManifold::default();
        manifold.fill(world, center_a, center_b, material, best.as_ref());
        manifold
    }

    #[inline]
    pub(crate) fn replace(
        &mut self,
        world: &Manifold,
        center_a: Vec3,
        center_b: Vec3,
        material: MixedMaterial,
    ) {
        let carry = (self.material.user_a == material.user_a
            && self.material.user_b == material.user_b)
            .then(|| Carry::from_manifold(self));
        self.fill(world, center_a, center_b, material, carry.as_ref());
    }

    #[inline]
    fn fill(
        &mut self,
        world: &Manifold,
        center_a: Vec3,
        center_b: Vec3,
        material: MixedMaterial,
        previous: Option<&Carry>,
    ) {
        self.normal = world.normal;
        self.count = world.count;
        self.material = material;
        self.friction_impulse = Vec3::ZERO;
        self.twist_impulse = 0.0;
        self.rolling_impulse = Vec3::ZERO;
        let mut matched = false;
        for (target, source) in self.points.iter_mut().zip(world.points()) {
            let inherited = previous.and_then(|carry| carry.impulse(source.feature));
            matched |= inherited.is_some();
            *target = ContactPoint {
                anchor_a: source.point - center_a,
                anchor_b: source.point - center_b,
                point: source.point,
                separation: source.separation,
                base_separation: source.separation,
                normal_impulse: inherited.unwrap_or(0.0),
                total_normal_impulse: 0.0,
                normal_velocity: 0.0,
                feature: source.feature,
                triangle: source.triangle,
                persisted: inherited.is_some(),
            };
        }
        if let Some(carry) = previous
            && matched
            && carry.normal.dot(self.normal) > 0.95
        {
            self.friction_impulse = carry.friction_impulse;
            self.twist_impulse = carry.twist_impulse;
            self.rolling_impulse = carry.rolling_impulse;
        }
    }
}

#[derive(Clone, Copy)]
struct Carry {
    normal: Vec3,
    count: usize,
    features: [u32; MAX_MANIFOLD_POINTS],
    impulses: [f32; MAX_MANIFOLD_POINTS],
    friction_impulse: Vec3,
    twist_impulse: f32,
    rolling_impulse: Vec3,
}

impl Carry {
    #[inline]
    fn from_manifold(manifold: &ContactManifold) -> Self {
        let mut features = [0; MAX_MANIFOLD_POINTS];
        let mut impulses = [0.0; MAX_MANIFOLD_POINTS];
        for (index, point) in manifold.points().iter().enumerate() {
            features[index] = point.feature;
            impulses[index] = point.normal_impulse;
        }
        Self {
            normal: manifold.normal,
            count: manifold.count as usize,
            features,
            impulses,
            friction_impulse: manifold.friction_impulse,
            twist_impulse: manifold.twist_impulse,
            rolling_impulse: manifold.rolling_impulse,
        }
    }

    #[inline]
    fn impulse(&self, feature: u32) -> Option<f32> {
        self.features[..self.count]
            .iter()
            .position(|&old| old == feature)
            .map(|index| self.impulses[index])
    }
}
