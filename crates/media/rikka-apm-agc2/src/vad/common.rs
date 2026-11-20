pub(crate) const FRAME_10MS_24K: usize = 240;
pub(crate) const FRAME_20MS_24K: usize = 480;
pub(crate) const MIN_PITCH_24K: usize = 30;
pub(crate) const MAX_PITCH_24K: usize = 384;
pub(crate) const BUF_24K: usize = MAX_PITCH_24K + FRAME_20MS_24K;
pub(crate) const INITIAL_MIN_PITCH_24K: usize = 3 * MIN_PITCH_24K;
pub(crate) const INITIAL_NUM_LAGS_24K: usize = MAX_PITCH_24K - INITIAL_MIN_PITCH_24K;
pub(crate) const REFINE_NUM_LAGS_24K: usize = MAX_PITCH_24K + 1;

pub(crate) const FRAME_20MS_12K: usize = 240;
pub(crate) const BUF_12K: usize = BUF_24K / 2;
pub(crate) const MAX_PITCH_12K: usize = MAX_PITCH_24K / 2;
pub(crate) const NUM_LAGS_12K: usize = MAX_PITCH_12K - INITIAL_MIN_PITCH_24K / 2;

pub(crate) const MIN_PITCH_48K: usize = MIN_PITCH_24K * 2;
pub(crate) const MAX_PITCH_48K: usize = MAX_PITCH_24K * 2;

pub(crate) const NUM_BANDS: usize = 22;
pub(crate) const NUM_LOWER_BANDS: usize = 6;
pub(crate) const CEPSTRAL_HISTORY: usize = 8;
pub(crate) const FEATURE_VECTOR: usize = 42;

pub(crate) fn dot(x: &[f32], y: &[f32]) -> f32 {
    x.iter().zip(y).fold(0.0, |sum, (a, b)| sum + a * b)
}
