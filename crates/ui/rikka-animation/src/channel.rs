use std::time::Duration;

use crate::motion::Curve;

const REST_DISTANCE: f32 = 1e-3;

const REST_SPEED: f32 = 1e-2;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Channel {
    pub value: f32,
    pub velocity: f32,
    origin: f32,
    target: f32,
    elapsed: f32,
    gap: f32,
}

impl Channel {
    pub const fn new(value: f32) -> Self {
        Self {
            value,
            velocity: 0.0,
            origin: value,
            target: value,
            elapsed: 0.0,
            gap: 0.0,
        }
    }

    pub(crate) fn gap_to(&self, target: f32) -> f32 {
        match self.value == target + self.gap {
            true => self.gap,
            false => self.value - target,
        }
    }

    pub(crate) fn place(&mut self, target: f32, gap: f32, velocity: f32) {
        self.gap = gap;
        self.value = target + gap;
        self.velocity = velocity;
    }

    pub const fn target(&self) -> f32 {
        self.target
    }

    pub fn snap(&mut self, value: f32) {
        *self = Self::new(value);
    }

    pub fn shift(&mut self, by: f32) {
        self.value += by;
        self.origin += by;
        self.target += by;
    }

    pub fn is_resting(&self) -> bool {
        self.value == self.target && self.velocity == 0.0
    }

    #[inline]
    pub fn advance(&mut self, curve: Curve, target: f32, elapsed: Duration) {
        self.advance_spanning(curve, target, elapsed, 1.0);
    }

    #[inline]
    pub(crate) fn advance_spanning(
        &mut self,
        curve: Curve,
        target: f32,
        elapsed: Duration,
        span: f32,
    ) {
        if target != self.target {
            self.origin = self.value;
            self.target = target;
            self.elapsed = 0.0;
        }
        if self.is_resting() {
            return;
        }
        match curve {
            Curve::Spring(spring) => {
                spring.step(self, target, elapsed);
                if self.gap_to(target).abs() < REST_DISTANCE && self.velocity.abs() < REST_SPEED {
                    self.settle();
                }
            }
            Curve::Tween { easing, duration } => {
                let dt = elapsed.as_secs_f32();
                self.elapsed += dt;
                let seconds = duration.seconds() * span;
                if seconds <= 0.0 || self.elapsed >= seconds {
                    self.settle();
                    return;
                }
                let previous = self.value;
                self.value =
                    self.origin + (target - self.origin) * easing.transform(self.elapsed / seconds);
                if dt > 0.0 {
                    self.velocity = (self.value - previous) / dt;
                }
            }
        }
    }

    fn settle(&mut self) {
        self.value = self.target;
        self.velocity = 0.0;
        self.origin = self.target;
        self.elapsed = 0.0;
        self.gap = 0.0;
    }
}
