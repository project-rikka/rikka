pub(crate) const MIN_FLOAT_S16: f32 = -32768.0;
pub(crate) const MAX_FLOAT_S16: f32 = 32767.0;
pub(crate) const MIN_LEVEL_DBFS: f32 = -90.31;
pub(crate) const FRAME_DURATION_MS: i32 = 10;
pub(crate) const SUB_FRAMES_IN_FRAME: usize = 20;
pub(crate) const LIMITER_THRESHOLD_FOR_AGC_GAIN_DBFS: f32 = -1.0;
pub(crate) const VAD_RESET_PERIOD_MS: i32 = 1500;
pub(crate) const VAD_CONFIDENCE_THRESHOLD: f32 = 0.95;
pub(crate) const ADJACENT_SPEECH_FRAMES_THRESHOLD: i32 = 12;
pub(crate) const LEVEL_ESTIMATOR_TIME_TO_CONFIDENCE_MS: i32 = 400;
pub(crate) const LEVEL_ESTIMATOR_LEAK_FACTOR: f32 =
    1.0 - 1.0 / LEVEL_ESTIMATOR_TIME_TO_CONFIDENCE_MS as f32;
pub(crate) const SATURATION_PROTECTOR_INITIAL_HEADROOM_DB: f32 = 20.0;

const MIN_DBFS: f32 = -90.309;

pub(crate) fn db_to_ratio(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

pub(crate) fn float_s16_to_dbfs(level: f32) -> f32 {
    if level <= 1.0 {
        return MIN_DBFS;
    }
    20.0 * level.log10() + MIN_DBFS
}

pub(crate) fn energy_to_dbfs(energy: f32, samples: usize) -> f32 {
    let rms_square = energy / samples as f32;
    if rms_square <= 1.0 {
        return MIN_DBFS;
    }
    10.0 * rms_square.log10() + MIN_DBFS
}
