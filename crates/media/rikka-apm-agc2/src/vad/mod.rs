mod common;
mod features;
mod resampler;
mod rnn;

use features::FeaturesExtractor;
use resampler::Resampler;
use rikka_apm_audio::FRAME;
use rnn::Rnn;

use crate::common::{FRAME_DURATION_MS, VAD_RESET_PERIOD_MS};
use common::{FEATURE_VECTOR, FRAME_10MS_24K};

#[derive(Clone, Debug)]
pub(crate) struct VoiceActivityDetector {
    reset_period_frames: i32,
    time_to_reset: i32,
    resampler: Resampler,
    resampled: [f32; FRAME_10MS_24K],
    features: FeaturesExtractor,
    rnn: Rnn,
}

impl VoiceActivityDetector {
    pub(crate) fn new() -> VoiceActivityDetector {
        let reset_period_frames = VAD_RESET_PERIOD_MS / FRAME_DURATION_MS;
        VoiceActivityDetector {
            reset_period_frames,
            time_to_reset: reset_period_frames,
            resampler: Resampler::new(FRAME, FRAME_10MS_24K),
            resampled: [0.0; FRAME_10MS_24K],
            features: FeaturesExtractor::new(),
            rnn: Rnn::new(),
        }
    }

    pub(crate) fn analyze(&mut self, samples: &[f32; FRAME]) -> f32 {
        self.time_to_reset -= 1;
        if self.time_to_reset <= 0 {
            self.rnn.reset();
            self.time_to_reset = self.reset_period_frames;
        }
        self.resampler.resample(samples, &mut self.resampled);
        let mut feature_vector = [0.0f32; FEATURE_VECTOR];
        let is_silence = self.features.compute(&self.resampled, &mut feature_vector);
        self.rnn.probability(&feature_vector, is_silence)
    }
}
