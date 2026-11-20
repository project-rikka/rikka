use std::time::Duration;

use crate::RATE;

pub(crate) fn energy(samples: &[f32]) -> f32 {
    samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len().max(1) as f32
}

pub(crate) fn frames_duration(frames: usize) -> Duration {
    Duration::from_secs_f64(frames as f64 / f64::from(RATE))
}

pub(crate) fn frames_ms(frames: usize) -> f64 {
    frames_duration(frames).as_secs_f64() * 1000.0
}
