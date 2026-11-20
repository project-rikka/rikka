use rikka_model_core::glam::{Quat, Vec3};

use crate::muscle::MUSCLE_COUNT;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HumanPose {
    pub root_pos: Vec3,
    pub root_rot: Quat,
    pub muscles: [f32; MUSCLE_COUNT],
}

impl Default for HumanPose {
    fn default() -> Self {
        Self {
            root_pos: Vec3::ZERO,
            root_rot: Quat::IDENTITY,
            muscles: [0.0; MUSCLE_COUNT],
        }
    }
}
