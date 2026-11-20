use rikka_physics_math::{Quat, Transform, Vec3};

use crate::solver::sym::Sym3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BodyPose {
    pub transform: Transform,
    pub center: Vec3,
    pub local_center: Vec3,
    pub center0: Vec3,
    pub rotation0: Quat,
    pub extent: f32,
    pub reach: Vec3,
    pub min_extent: f32,
}

impl BodyPose {
    pub(crate) fn new(transform: Transform, local_center: Vec3) -> Self {
        let center = transform.apply(local_center);
        Self {
            transform,
            center,
            local_center,
            center0: center,
            rotation0: transform.q,
            extent: 0.0,
            reach: Vec3::ZERO,
            min_extent: 0.0,
        }
    }

    pub(crate) fn set_center(&mut self, center: Vec3, rotation: Quat) {
        self.center = center;
        self.transform.q = rotation;
        self.transform.p = center - rotation * self.local_center;
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct BodyMotion {
    pub mass: f32,
    pub inv_mass: f32,
    pub inv_inertia: Sym3,
    pub inertia_local: Sym3,
    pub inv_inertia_local: Sym3,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub force: Vec3,
    pub torque: Vec3,
}

impl BodyMotion {
    pub(crate) fn refresh_world_inertia(&mut self, rotation: Quat) {
        self.inv_inertia = self
            .inv_inertia_local
            .rotated(rikka_physics_math::Mat3::from_quat(rotation));
    }
}
