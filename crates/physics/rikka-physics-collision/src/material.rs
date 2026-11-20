use rikka_physics_math::{Quat, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    pub friction: f32,
    pub restitution: f32,
    pub rolling_resistance: f32,
    pub tangent_velocity: Vec3,
    pub user_id: u32,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            friction: 0.6,
            restitution: 0.0,
            rolling_resistance: 0.0,
            tangent_velocity: Vec3::ZERO,
            user_id: 0,
        }
    }
}

impl Material {
    #[must_use]
    pub fn friction(mut self, friction: f32) -> Self {
        self.friction = friction;
        self
    }

    #[must_use]
    pub fn restitution(mut self, restitution: f32) -> Self {
        self.restitution = restitution;
        self
    }

    #[must_use]
    pub fn rolling_resistance(mut self, rolling_resistance: f32) -> Self {
        self.rolling_resistance = rolling_resistance;
        self
    }

    #[must_use]
    pub fn tangent_velocity(mut self, tangent_velocity: Vec3) -> Self {
        self.tangent_velocity = tangent_velocity;
        self
    }

    #[must_use]
    pub fn user_id(mut self, user_id: u32) -> Self {
        self.user_id = user_id;
        self
    }

    pub fn is_valid(&self) -> bool {
        self.friction.is_finite()
            && self.friction >= 0.0
            && self.restitution.is_finite()
            && self.restitution >= 0.0
            && self.rolling_resistance.is_finite()
            && self.rolling_resistance >= 0.0
            && self.tangent_velocity.is_finite()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MixedMaterial {
    pub friction: f32,
    pub restitution: f32,
    pub rolling_resistance: f32,
    pub tangent_velocity: Vec3,
    pub user_a: u32,
    pub user_b: u32,
}

impl MixedMaterial {
    pub fn mix(a: &Material, rotation_a: Quat, b: &Material, rotation_b: Quat) -> Self {
        Self {
            friction: (a.friction * b.friction).sqrt(),
            restitution: a.restitution.max(b.restitution),
            rolling_resistance: a.rolling_resistance.max(b.rolling_resistance),
            tangent_velocity: surface_velocity(rotation_a, a.tangent_velocity)
                - surface_velocity(rotation_b, b.tangent_velocity),
            user_a: a.user_id,
            user_b: b.user_id,
        }
    }
}

#[inline]
fn surface_velocity(rotation: Quat, velocity: Vec3) -> Vec3 {
    if velocity == Vec3::ZERO {
        Vec3::ZERO
    } else {
        rotation * velocity
    }
}
