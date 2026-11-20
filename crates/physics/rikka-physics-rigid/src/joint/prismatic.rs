use std::ops::RangeInclusive;

use rikka_physics_math::Vec3;

use super::axes::Axes;
use super::builder::Joint;
use super::motion::{Axis, Motion};
use super::spring::Spring;
use crate::BodyId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prismatic {
    axes: Axes,
}

impl Prismatic {
    pub fn axes(&self) -> &Axes {
        &self.axes
    }

    pub fn new(body_a: BodyId, body_b: BodyId) -> Joint<Prismatic> {
        let mut axes = Axes::uniform(Motion::Locked);
        axes.set_motion(Axis::LinearZ, Motion::Free);
        Joint::new(body_a, body_b, Prismatic { axes })
    }

    pub fn set_motor_speed(&mut self, speed: f32) {
        self.axes.set_velocity(Axis::LinearZ, speed);
    }

    pub fn set_target_translation(&mut self, translation: f32) {
        self.axes
            .set_target_translation(Vec3::new(0.0, 0.0, translation));
    }
}

impl Joint<Prismatic> {
    pub fn limits(mut self, translation: RangeInclusive<f32>) -> Self {
        self.params.axes.set_motion(
            Axis::LinearZ,
            Motion::Limited {
                lower: *translation.start(),
                upper: *translation.end(),
            },
        );
        self
    }

    pub fn spring(mut self, spring: Spring) -> Self {
        self.params.axes.set_spring(Axis::LinearZ, spring);
        self
    }

    pub fn target_translation(mut self, translation: f32) -> Self {
        self.params.set_target_translation(translation);
        self
    }

    pub fn motor(mut self, speed: f32, max_force: f32) -> Self {
        self.params.axes.set_motor(Axis::LinearZ, max_force);
        self.params.set_motor_speed(speed);
        self
    }
}

impl rikka_file_format::Codec for Prismatic {
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
