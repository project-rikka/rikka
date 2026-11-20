use std::ops::RangeInclusive;

use rikka_physics_math::Quat;

use super::axes::Axes;
use super::builder::Joint;
use super::drive::Drive;
use super::motion::{Axis, Motion};
use super::spring::Spring;
use crate::BodyId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wheel {
    axes: Axes,
}

impl Wheel {
    pub fn axes(&self) -> &Axes {
        &self.axes
    }

    pub fn new(body_a: BodyId, body_b: BodyId) -> Joint<Wheel> {
        let mut axes = Axes::uniform(Motion::Locked);
        axes.set_motion(Axis::LinearY, Motion::Free);
        axes.set_motion(Axis::Twist, Motion::Free);
        axes.set_drive(
            Axis::LinearY,
            Drive::spring(Spring::hertz(1.0).damping_ratio(0.7)),
        );
        Joint::new(body_a, body_b, Wheel { axes })
    }

    pub fn set_spin_speed(&mut self, speed: f32) {
        self.axes.set_velocity(Axis::Twist, speed);
    }

    pub fn set_steering_angle(&mut self, angle: f32) {
        self.axes.set_target_rotation(Quat::from_rotation_y(angle));
    }
}

impl Joint<Wheel> {
    pub fn suspension(mut self, spring: Spring) -> Self {
        self.params.axes.set_spring(Axis::LinearY, spring);
        self
    }

    pub fn suspension_limits(mut self, travel: RangeInclusive<f32>) -> Self {
        self.params.axes.set_motion(
            Axis::LinearY,
            Motion::Limited {
                lower: *travel.start(),
                upper: *travel.end(),
            },
        );
        self
    }

    pub fn spin_motor(mut self, speed: f32, max_torque: f32) -> Self {
        self.params.axes.set_motor(Axis::Twist, max_torque);
        self.params.set_spin_speed(speed);
        self
    }

    pub fn steering(mut self, spring: Spring, max_torque: f32) -> Self {
        let axes = &mut self.params.axes;
        if axes.motion(Axis::SwingY).is_locked() {
            axes.set_motion(Axis::SwingY, Motion::Free);
        }
        axes.set_drive(Axis::SwingY, Drive::spring(spring).max_effort(max_torque));
        self
    }

    pub fn steering_limits(mut self, angles: RangeInclusive<f32>) -> Self {
        self.params.axes.set_motion(
            Axis::SwingY,
            Motion::Limited {
                lower: *angles.start(),
                upper: *angles.end(),
            },
        );
        self
    }

    pub fn steering_angle(mut self, angle: f32) -> Self {
        self.params.set_steering_angle(angle);
        self
    }
}

impl rikka_file_format::Codec for Wheel {
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
