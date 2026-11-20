use rikka_physics_math::{Transform, Vec3};

use super::axes::Axes;
use super::builder::Joint;
use super::drive::Drive;
use super::motion::{Axis, Motion};
use crate::BodyId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D6 {
    axes: Axes,
}

impl D6 {
    pub fn axes(&self) -> &Axes {
        &self.axes
    }

    pub fn new(body_a: BodyId, body_b: BodyId) -> Joint<D6> {
        Joint::new(
            body_a,
            body_b,
            D6 {
                axes: Axes::uniform(Motion::Locked),
            },
        )
    }

    pub fn set_target(&mut self, target: Transform) {
        self.axes.set_target(target);
    }

    pub fn set_target_velocity(&mut self, linear: Vec3, angular: Vec3) {
        self.axes.set_linear_velocity(linear);
        self.axes.set_angular_velocity(angular);
    }

    pub fn target(&self) -> Transform {
        self.axes.target()
    }
}

impl Joint<D6> {
    pub fn motion(mut self, axis: Axis, motion: Motion) -> Self {
        self.params.axes.set_motion(axis, motion);
        self
    }

    pub fn linear(mut self, motion: Motion) -> Self {
        for axis in Axis::LINEAR {
            self.params.axes.set_motion(axis, motion);
        }
        self
    }

    pub fn angular(mut self, motion: Motion) -> Self {
        for axis in Axis::ANGULAR {
            self.params.axes.set_motion(axis, motion);
        }
        self
    }

    pub fn cone(mut self, angle_x: f32, angle_y: f32) -> Self {
        self.params.axes.set_cone(angle_x, angle_y);
        self
    }

    pub fn drive(mut self, axis: Axis, drive: Drive) -> Self {
        self.params.axes.set_drive(axis, drive);
        self
    }

    pub fn motor(mut self, axis: Axis, max_effort: f32) -> Self {
        self.params.axes.set_motor(axis, max_effort);
        self
    }

    pub fn pose_drive(mut self, linear: Drive, angular: Drive) -> Self {
        self.params.axes.set_linear_pose(linear);
        self.params.axes.set_angular_pose(angular);
        self
    }

    pub fn linear_pose_drive(mut self, drive: Drive) -> Self {
        self.params.axes.set_linear_pose(drive);
        self
    }

    pub fn angular_pose_drive(mut self, drive: Drive) -> Self {
        self.params.axes.set_angular_pose(drive);
        self
    }

    pub fn target(mut self, target: Transform) -> Self {
        self.params.set_target(target);
        self
    }

    pub fn target_velocity(mut self, linear: Vec3, angular: Vec3) -> Self {
        self.params.set_target_velocity(linear, angular);
        self
    }
}

impl rikka_file_format::Codec for D6 {
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
