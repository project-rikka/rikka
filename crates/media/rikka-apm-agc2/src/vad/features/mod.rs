mod lpc;
mod pitch;
mod spectral;

use pitch::PitchEstimator;
use spectral::SpectralFeatures;

use crate::vad::common::{BUF_24K, FEATURE_VECTOR, FRAME_10MS_24K, FRAME_20MS_24K, MAX_PITCH_24K};

#[derive(Clone, Debug)]
pub(crate) struct FeaturesExtractor {
    pitch_buffer: [f32; BUF_24K],
    residual: [f32; BUF_24K],
    pitch: PitchEstimator,
    spectral: SpectralFeatures,
}

impl FeaturesExtractor {
    pub(crate) fn new() -> FeaturesExtractor {
        FeaturesExtractor {
            pitch_buffer: [0.0; BUF_24K],
            residual: [0.0; BUF_24K],
            pitch: PitchEstimator::new(),
            spectral: SpectralFeatures::new(),
        }
    }

    pub(crate) fn compute(
        &mut self,
        samples: &[f32; FRAME_10MS_24K],
        features: &mut [f32; FEATURE_VECTOR],
    ) -> bool {
        self.pitch_buffer.copy_within(FRAME_10MS_24K.., 0);
        self.pitch_buffer[BUF_24K - FRAME_10MS_24K..].copy_from_slice(samples);

        let lpc = lpc::lpc_coefficients(&self.pitch_buffer);
        lpc::lp_residual(&lpc, &self.pitch_buffer, &mut self.residual);
        let period_48k = self.pitch.estimate(&self.residual);
        features[FEATURE_VECTOR - 2] = 0.01 * (period_48k as i32 - 300) as f32;

        let lag_start = MAX_PITCH_24K - period_48k / 2;
        self.spectral.compute(
            &self.pitch_buffer[BUF_24K - FRAME_20MS_24K..],
            &self.pitch_buffer[lag_start..lag_start + FRAME_20MS_24K],
            features,
        )
    }
}
