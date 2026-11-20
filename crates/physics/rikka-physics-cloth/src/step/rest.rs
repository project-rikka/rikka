use crate::readback::Movement;

const STILL_STEPS: f32 = 30.0;
const STILL_DAMPING: f32 = 0.5;
const SLOW_SPEED: f32 = 0.1;
const FAST_SPEED: f32 = 0.2;
const SETTLED_SPEED: f32 = 0.03;
const WINDOW_STEPS: u32 = 15;
const SETTLING: f32 = 0.6;
const STEADY_FROM: f32 = 10.0;
const STEADY_STEPS: f32 = 20.0;
const MAX_FREEZE: f32 = 0.95;
const FREEZE_RELEASE: f32 = 0.4;
const LOCAL_AFTER: u32 = 2 * WINDOW_STEPS;
const LOCAL_SHARE: f32 = 0.01;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Rest {
    still: u32,
    peak: f32,
    lowest: Option<f32>,
    jitter: f32,
    moving: f32,
    freeze: f32,
    asleep: bool,
}

impl Rest {
    pub(crate) fn observe(&mut self, still: bool, movement: Movement) {
        if !still {
            *self = Rest::default();
            return;
        }
        self.still = self.still.saturating_add(1);
        if self.asleep {
            return;
        }
        self.peak = self.peak.max(movement.speed);
        self.jitter = self.jitter.max(movement.jitter);
        self.moving = self.moving.max(movement.moving);
        if !self.still.is_multiple_of(WINDOW_STEPS) {
            return;
        }
        let rebounding = self
            .lowest
            .is_some_and(|lowest| self.peak >= lowest * SETTLING);
        if self.peak < SETTLED_SPEED || (self.peak < SLOW_SPEED && rebounding) {
            self.asleep = true;
        } else if self.still > WINDOW_STEPS && self.is_jittering() {
            self.freeze = (1.0 - (1.0 - self.freeze) * FREEZE_RELEASE).min(MAX_FREEZE);
        }
        self.lowest = Some(
            self.lowest
                .map_or(self.peak, |lowest| lowest.min(self.peak)),
        );
        self.peak = 0.0;
        self.jitter = 0.0;
        self.moving = 0.0;
    }

    fn is_jittering(&self) -> bool {
        let local = self.peak >= SLOW_SPEED && self.moving <= LOCAL_SHARE;
        self.jitter >= SLOW_SPEED || (self.still > LOCAL_AFTER && local)
    }

    pub(crate) fn asleep(&self) -> bool {
        self.asleep
    }

    pub(crate) fn activity(&self) -> f32 {
        1.0 - (self.still as f32 / STILL_STEPS).min(1.0)
    }

    pub(crate) fn stiffening(&self) -> f32 {
        MAX_FREEZE * ((self.still as f32 - STEADY_FROM) / STEADY_STEPS).clamp(0.0, 1.0)
    }

    pub(crate) fn freeze(&self) -> f32 {
        self.freeze
    }

    pub(crate) fn damping(&self, motion: f32) -> f32 {
        let still_damping = ((FAST_SPEED - motion) / (FAST_SPEED - SLOW_SPEED)).clamp(0.0, 1.0);
        STILL_DAMPING * (1.0 - self.activity()) * still_damping
    }
}
