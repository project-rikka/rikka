use rikka_physics_geometry::MassData;
use rikka_physics_math::{Quat, Transform, Vec3};

use crate::shape::ShapeDef;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BodyKind {
    Static,
    Kinematic,
    Dynamic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AxisLocks {
    pub linear: [bool; 3],
    pub angular: [bool; 3],
}

impl AxisLocks {
    pub const NONE: Self = Self {
        linear: [false; 3],
        angular: [false; 3],
    };

    pub const ROTATION: Self = Self {
        linear: [false; 3],
        angular: [true; 3],
    };

    pub(crate) fn bits(&self) -> u32 {
        let mut bits = 0;
        for axis in 0..3 {
            bits |= (self.linear[axis] as u32) << axis;
            bits |= (self.angular[axis] as u32) << (axis + 3);
        }
        bits
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MassSpec {
    FromShapes,
    Total(f32),
    Explicit(MassData),
}

#[must_use]
#[derive(Clone, Debug)]
pub struct Body {
    pub(crate) kind: BodyKind,
    pub(crate) pose: Transform,
    pub(crate) linear_velocity: Vec3,
    pub(crate) angular_velocity: Vec3,
    pub(crate) linear_damping: f32,
    pub(crate) angular_damping: f32,
    pub(crate) gravity_scale: f32,
    pub(crate) locks: AxisLocks,
    pub(crate) bullet: bool,
    pub(crate) sleep: bool,
    pub(crate) sleep_threshold: Option<f32>,
    pub(crate) awake: bool,
    pub(crate) character: bool,
    pub(crate) fast_rotation: bool,
    pub(crate) user_data: u64,
    pub(crate) mass: MassSpec,
    pub(crate) shapes: Vec<ShapeDef>,
}

impl Body {
    fn new(kind: BodyKind) -> Self {
        Self {
            kind,
            pose: Transform::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            linear_damping: 0.0,
            angular_damping: 0.0,
            gravity_scale: 1.0,
            locks: AxisLocks::NONE,
            bullet: false,
            sleep: true,
            sleep_threshold: None,
            awake: true,
            character: false,
            fast_rotation: false,
            user_data: 0,
            mass: MassSpec::FromShapes,
            shapes: Vec::new(),
        }
    }

    pub fn dynamic() -> Self {
        Self::new(BodyKind::Dynamic)
    }

    pub fn kinematic() -> Self {
        Self::new(BodyKind::Kinematic)
    }

    pub fn fixed() -> Self {
        Self::new(BodyKind::Static)
    }

    pub fn pose(mut self, pose: Transform) -> Self {
        self.pose = pose;
        self
    }

    pub fn position(mut self, position: Vec3) -> Self {
        self.pose.p = position;
        self
    }

    pub fn rotation(mut self, rotation: Quat) -> Self {
        self.pose.q = rotation;
        self
    }

    pub fn shape(mut self, shape: impl Into<ShapeDef>) -> Self {
        self.shapes.push(shape.into());
        self
    }

    pub fn mass(mut self, mass: f32) -> Self {
        self.mass = MassSpec::Total(mass);
        self
    }

    pub fn mass_data(mut self, mass: MassData) -> Self {
        self.mass = MassSpec::Explicit(mass);
        self
    }

    pub fn linear_velocity(mut self, velocity: Vec3) -> Self {
        self.linear_velocity = velocity;
        self
    }

    pub fn angular_velocity(mut self, velocity: Vec3) -> Self {
        self.angular_velocity = velocity;
        self
    }

    pub fn damping(mut self, linear: f32, angular: f32) -> Self {
        self.linear_damping = linear;
        self.angular_damping = angular;
        self
    }

    pub fn gravity_scale(mut self, scale: f32) -> Self {
        self.gravity_scale = scale;
        self
    }

    pub fn locks(mut self, locks: AxisLocks) -> Self {
        self.locks = locks;
        self
    }

    pub fn bullet(mut self, bullet: bool) -> Self {
        self.bullet = bullet;
        self
    }

    pub fn sleep(mut self, enabled: bool) -> Self {
        self.sleep = enabled;
        self
    }

    pub fn sleep_threshold(mut self, speed: f32) -> Self {
        self.sleep_threshold = Some(speed);
        self
    }

    pub fn awake(mut self, awake: bool) -> Self {
        self.awake = awake;
        self
    }

    pub fn character(mut self) -> Self {
        self.character = true;
        self.sleep = false;
        self
    }

    pub fn fast_rotation(mut self, allowed: bool) -> Self {
        self.fast_rotation = allowed;
        self
    }

    pub fn user_data(mut self, user_data: u64) -> Self {
        self.user_data = user_data;
        self
    }
}
