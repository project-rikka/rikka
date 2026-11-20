use std::ops::RangeInclusive;

use rikka_physics_math::Quat;

use super::axes::Axes;
use super::builder::Joint;
use super::motion::{Axis, Motion};
use super::spring::Spring;
use crate::BodyId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Revolute {
    axes: Axes,
}

impl Revolute {
    pub fn axes(&self) -> &Axes {
        &self.axes
    }

    pub fn new(body_a: BodyId, body_b: BodyId) -> Joint<Revolute> {
        let mut axes = Axes::uniform(Motion::Locked);
        axes.set_motion(Axis::Twist, Motion::Free);
        Joint::new(body_a, body_b, Revolute { axes })
    }

    pub fn set_motor_speed(&mut self, speed: f32) {
        self.axes.set_velocity(Axis::Twist, speed);
    }

    pub fn set_target_angle(&mut self, angle: f32) {
        self.axes.set_target_rotation(Quat::from_rotation_z(angle));
    }
}

impl Joint<Revolute> {
    pub fn limits(mut self, angles: RangeInclusive<f32>) -> Self {
        self.params.axes.set_motion(
            Axis::Twist,
            Motion::Limited {
                lower: *angles.start(),
                upper: *angles.end(),
            },
        );
        self
    }

    pub fn spring(mut self, spring: Spring) -> Self {
        self.params.axes.set_spring(Axis::Twist, spring);
        self
    }

    pub fn target_angle(mut self, angle: f32) -> Self {
        self.params.set_target_angle(angle);
        self
    }

    pub fn motor(mut self, speed: f32, max_torque: f32) -> Self {
        self.params.axes.set_motor(Axis::Twist, max_torque);
        self.params.set_motor_speed(speed);
        self
    }
}

impl rikka_file_format::Codec for Revolute {
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
