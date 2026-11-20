mod gain_curve;
mod level_estimator;

use level_estimator::LevelEstimator;
use rikka_apm_audio::{FRAME, Frame};

use crate::common::{MAX_FLOAT_S16, MIN_FLOAT_S16, SUB_FRAMES_IN_FRAME};

const SUB_FRAME: usize = FRAME / SUB_FRAMES_IN_FRAME;
const ATTACK_FIRST_SUB_FRAME_INTERPOLATION_POWER: f32 = 8.0;

#[derive(Clone, Debug)]
pub(crate) struct Limiter {
    level_estimator: LevelEstimator,
    scaling_factors: [f32; SUB_FRAMES_IN_FRAME + 1],
    per_sample_scaling_factors: [f32; FRAME],
    last_scaling_factor: f32,
}

impl Limiter {
    pub(crate) fn new() -> Limiter {
        Limiter {
            level_estimator: LevelEstimator::new(),
            scaling_factors: [0.0; SUB_FRAMES_IN_FRAME + 1],
            per_sample_scaling_factors: [0.0; FRAME],
            last_scaling_factor: 1.0,
        }
    }

    pub(crate) fn last_audio_level(&self) -> f32 {
        self.level_estimator.last_level()
    }

    pub(crate) fn last_scaling_factor(&self) -> f32 {
        self.last_scaling_factor
    }

    pub(crate) fn process(&mut self, frame: &mut Frame, channels: usize) {
        let levels = self.level_estimator.compute(frame, channels);
        self.scaling_factors[0] = self.last_scaling_factor;
        for (factor, level) in self.scaling_factors[1..].iter_mut().zip(levels) {
            *factor = gain_curve::gain_to_apply(level);
        }
        per_sample_factors(&self.scaling_factors, &mut self.per_sample_scaling_factors);
        for channel in 0..channels {
            for (sample, factor) in frame
                .full_mut(channel)
                .iter_mut()
                .zip(&self.per_sample_scaling_factors)
            {
                *sample = (*sample * factor).clamp(MIN_FLOAT_S16, MAX_FLOAT_S16);
            }
        }
        self.last_scaling_factor = self.scaling_factors[SUB_FRAMES_IN_FRAME];
    }
}

fn per_sample_factors(
    scaling_factors: &[f32; SUB_FRAMES_IN_FRAME + 1],
    per_sample: &mut [f32; FRAME],
) {
    let is_attack = scaling_factors[0] > scaling_factors[1];
    if is_attack {
        interpolate_first_sub_frame(
            scaling_factors[0],
            scaling_factors[1],
            &mut per_sample[..SUB_FRAME],
        );
    }
    let first = usize::from(is_attack);
    for (index, samples) in per_sample
        .as_chunks_mut::<SUB_FRAME>()
        .0
        .iter_mut()
        .enumerate()
        .skip(first)
    {
        let start = scaling_factors[index];
        let end = scaling_factors[index + 1];
        let step = (end - start) / SUB_FRAME as f32;
        for (offset, factor) in samples.iter_mut().enumerate() {
            *factor = start + step * offset as f32;
        }
    }
}

fn interpolate_first_sub_frame(last: f32, current: f32, sub_frame: &mut [f32]) {
    let length = sub_frame.len();
    for (index, factor) in sub_frame.iter_mut().enumerate() {
        let progress = (index / length) as f32;
        *factor = (1.0 - progress).powf(ATTACK_FIRST_SUB_FRAME_INTERPOLATION_POWER)
            * (last - current)
            + current;
    }
}
