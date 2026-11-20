mod auto_correlation;
mod search;

use auto_correlation::AutoCorrelation;
use search::PitchInfo;

use crate::vad::common::{BUF_12K, BUF_24K, MAX_PITCH_48K, NUM_LAGS_12K, REFINE_NUM_LAGS_24K};

#[derive(Clone, Debug)]
pub(crate) struct PitchEstimator {
    last: PitchInfo,
    auto_correlation: AutoCorrelation,
    energies_24k: [f32; REFINE_NUM_LAGS_24K],
    buffer_12k: [f32; BUF_12K],
    auto_corr_12k: [f32; NUM_LAGS_12K],
}

impl PitchEstimator {
    pub(crate) fn new() -> PitchEstimator {
        PitchEstimator {
            last: PitchInfo::default(),
            auto_correlation: AutoCorrelation::new(),
            energies_24k: [0.0; REFINE_NUM_LAGS_24K],
            buffer_12k: [0.0; BUF_12K],
            auto_corr_12k: [0.0; NUM_LAGS_12K],
        }
    }

    pub(crate) fn estimate(&mut self, pitch_buffer: &[f32; BUF_24K]) -> usize {
        search::decimate_2x(pitch_buffer, &mut self.buffer_12k);
        self.auto_correlation
            .compute(&self.buffer_12k, &mut self.auto_corr_12k);
        let mut candidates = search::pitch_period_12k(&self.buffer_12k, &self.auto_corr_12k);
        candidates.best *= 2;
        candidates.second_best *= 2;
        search::sliding_frame_energies_24k(pitch_buffer, &mut self.energies_24k);
        let lag_48k = search::pitch_period_48k(pitch_buffer, &self.energies_24k, candidates);
        self.last = search::extended_pitch_period_48k(
            pitch_buffer,
            &self.energies_24k,
            MAX_PITCH_48K - lag_48k,
            self.last,
        );
        self.last.period
    }
}
