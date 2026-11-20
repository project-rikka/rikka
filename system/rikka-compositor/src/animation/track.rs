use std::time::Duration;

use rikka_animation::{Channel, Curve};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Track {
    channel: Channel,
    curve: Curve,
}

impl Track {
    pub(crate) const fn new(value: f32, curve: Curve) -> Self {
        Self {
            channel: Channel::new(value),
            curve,
        }
    }

    pub(crate) fn snap(&mut self, value: f32) {
        self.channel.snap(value);
    }

    pub(crate) fn launch(&mut self, velocity: f32) {
        self.channel.velocity = velocity;
    }

    pub(crate) fn aim(&mut self, target: f32, curve: Curve) {
        self.curve = curve;
        self.channel.advance(curve, target, Duration::ZERO);
    }

    pub(crate) fn step(&mut self, elapsed: Duration) {
        self.channel
            .advance(self.curve, self.channel.target(), elapsed);
    }

    pub(crate) fn target(&self) -> f32 {
        self.channel.target()
    }

    pub(crate) fn value(&self) -> f32 {
        self.channel.value
    }

    pub(crate) fn is_resting(&self) -> bool {
        self.channel.is_resting()
    }
}
