use super::axes::Axes;
use super::builder::Joint;
use super::drive::Drive;
use super::motion::{Axis, Motion};
use super::spring::Spring;
use crate::BodyId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parallel {
    axes: Axes,
}

impl Parallel {
    pub fn axes(&self) -> &Axes {
        &self.axes
    }

    pub fn new(body_a: BodyId, body_b: BodyId) -> Joint<Parallel> {
        let mut axes = Axes::uniform(Motion::Free);
        let drive = Drive::spring(Spring::hertz(1.0).damping_ratio(1.0));
        axes.set_drive(Axis::SwingX, drive);
        axes.set_drive(Axis::SwingY, drive);
        Joint::new(body_a, body_b, Parallel { axes })
    }
}

impl Joint<Parallel> {
    pub fn spring(mut self, spring: Spring) -> Self {
        self.params.axes.set_spring(Axis::SwingX, spring);
        self.params.axes.set_spring(Axis::SwingY, spring);
        self
    }

    pub fn max_torque(mut self, torque: f32) -> Self {
        self.params.axes.set_max_effort(Axis::SwingX, torque);
        self.params.axes.set_max_effort(Axis::SwingY, torque);
        self
    }
}

impl rikka_file_format::Codec for Parallel {
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
