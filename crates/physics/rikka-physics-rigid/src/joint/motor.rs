use rikka_physics_math::{Transform, Vec3};

use super::axes::Axes;
use super::builder::Joint;
use super::drive::Drive;
use super::motion::Motion;
use super::spring::Spring;
use crate::BodyId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motor {
    axes: Axes,
}

impl Motor {
    pub fn axes(&self) -> &Axes {
        &self.axes
    }

    pub fn new(body_a: BodyId, body_b: BodyId) -> Joint<Motor> {
        let mut axes = Axes::uniform(Motion::Free);
        axes.set_linear_pose(Drive::velocity());
        axes.set_angular_pose(Drive::velocity());
        Joint::new(body_a, body_b, Motor { axes })
    }

    pub fn set_target(&mut self, target: Transform) {
        self.axes.set_target(target);
    }

    pub fn set_velocity(&mut self, linear: Vec3, angular: Vec3) {
        self.axes.set_linear_velocity(linear);
        self.axes.set_angular_velocity(angular);
    }
}

impl Joint<Motor> {
    pub fn linear_velocity(mut self, velocity: Vec3) -> Self {
        self.params.axes.set_linear_velocity(velocity);
        self
    }

    pub fn angular_velocity(mut self, velocity: Vec3) -> Self {
        self.params.axes.set_angular_velocity(velocity);
        self
    }

    pub fn max_force(mut self, force: f32) -> Self {
        self.params
            .axes
            .update_linear_pose(|drive| drive.max_effort = force);
        self
    }

    pub fn max_torque(mut self, torque: f32) -> Self {
        self.params
            .axes
            .update_angular_pose(|drive| drive.max_effort = torque);
        self
    }

    pub fn linear_spring(mut self, spring: Spring) -> Self {
        self.params
            .axes
            .update_linear_pose(|drive| drive.spring = Some(spring));
        self
    }

    pub fn angular_spring(mut self, spring: Spring) -> Self {
        self.params
            .axes
            .update_angular_pose(|drive| drive.spring = Some(spring));
        self
    }

    pub fn target(mut self, target: Transform) -> Self {
        self.params.set_target(target);
        self
    }
}

impl rikka_file_format::Codec for Motor {
    type Error = crate::Error;

    fn encode(&self, out: &mut rikka_file_format::Writer<'_>) {
        self.axes.encode(out);
    }

    fn decode(input: &mut rikka_file_format::Reader<'_>) -> crate::Result<Self> {
        Ok(Self {
            axes: Axes::decode(input)?,
        })
    }
}
