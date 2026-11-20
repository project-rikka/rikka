use std::time::Duration;

use discord_core::SAMPLE_RATE;

const CEILING: f32 = 0.89;
const RELEASE: Duration = Duration::from_millis(80);

#[derive(Debug)]
pub(crate) struct Limiter {
    gain: f32,
    release: f32,
}

impl Limiter {
    pub(crate) fn new() -> Limiter {
        let samples = RELEASE.as_secs_f32() * SAMPLE_RATE as f32;
        Limiter {
            gain: 1.0,
            release: 1.0 - (-1.0 / samples).exp(),
        }
    }

    pub(crate) fn process(&mut self, interleaved: &mut [f32], channels: usize) {
        for frame in interleaved.chunks_mut(channels.max(1)) {
            let peak = frame
                .iter()
                .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
            let allowed = if peak > CEILING { CEILING / peak } else { 1.0 };
            let released = self.gain + (1.0 - self.gain) * self.release;
            self.gain = released.min(allowed);
            for sample in frame {
                *sample *= self.gain;
            }
        }
    }
}
