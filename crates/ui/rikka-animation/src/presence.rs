use std::time::Duration;

use crate::channel::Channel;
use crate::motion::Curve;

const HIDDEN: f32 = 0.0;

const SHOWN: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presence {
    channel: Channel,
    span: f32,
}

impl Presence {
    pub const HIDDEN: Self = Self::new(HIDDEN, 0.0);

    pub const SHOWN: Self = Self::new(SHOWN, 0.0);

    pub const fn new(progress: f32, velocity: f32) -> Self {
        let mut channel = Channel::new(progress);
        channel.velocity = velocity;
        Self { channel, span: 1.0 }
    }

    pub const fn progress(&self) -> f32 {
        self.channel.value
    }

    pub fn is_resting(&self) -> bool {
        self.channel.is_resting()
    }

    pub fn advance(&mut self, shown: bool, curve: Curve, elapsed: Duration) {
        let target = match shown {
            true => SHOWN,
            false => HIDDEN,
        };
        if target != self.channel.target() {
            self.span = (target - self.channel.value).abs().min(1.0);
        }
        self.channel
            .advance_spanning(curve, target, elapsed, self.span);
    }
}
