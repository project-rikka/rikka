use rikka_apm_audio::{FRAME, Frame};

use crate::common::MAX_FLOAT_S16;

#[derive(Clone, Debug)]
pub(crate) struct GainApplier {
    last_factor: f32,
    factor: f32,
}

impl GainApplier {
    pub(crate) fn new(initial_factor: f32) -> GainApplier {
        GainApplier {
            last_factor: initial_factor,
            factor: initial_factor,
        }
    }

    pub(crate) fn set_factor(&mut self, factor: f32) {
        self.factor = factor;
    }

    pub(crate) fn apply(&mut self, frame: &mut Frame, channels: usize) {
        ramp(self.last_factor, self.factor, frame, channels);
        self.last_factor = self.factor;
    }
}

fn close_to_one(factor: f32) -> bool {
    (1.0 - 1.0 / MAX_FLOAT_S16..=1.0 + 1.0 / MAX_FLOAT_S16).contains(&factor)
}

fn ramp(last: f32, end: f32, frame: &mut Frame, channels: usize) {
    if last == end && close_to_one(end) {
        return;
    }
    if last == end {
        for channel in 0..channels {
            for sample in frame.full_mut(channel).iter_mut() {
                *sample *= end;
            }
        }
        return;
    }
    let increment = (end - last) * (1.0 / FRAME as f32);
    for channel in 0..channels {
        let mut gain = last;
        for sample in frame.full_mut(channel).iter_mut() {
            *sample *= gain;
            gain += increment;
        }
    }
}
