use rikka_apm_audio::BAND;

pub(crate) const FFT_SIZE: usize = 256;
pub(crate) const BINS: usize = FFT_SIZE / 2 + 1;
pub(crate) const FRAME_SIZE: usize = BAND;
pub(crate) const OVERLAP: usize = FFT_SIZE - FRAME_SIZE;

pub(crate) const SHORT_STARTUP_BLOCKS: i32 = 50;
pub(crate) const LONG_STARTUP_BLOCKS: i32 = 200;
pub(crate) const FEATURE_UPDATE_WINDOW: i32 = 500;

pub(crate) const LRT_FEATURE_THRESHOLD: f32 = 0.5;
pub(crate) const BIN_SIZE_LRT: f32 = 0.1;
pub(crate) const BIN_SIZE_FLATNESS: f32 = 0.05;
pub(crate) const BIN_SIZE_DIFF: f32 = 0.1;

pub(crate) const ONE_BY_BINS: f32 = 1.0 / BINS as f32;

pub(crate) type Spectrum = [f32; BINS];
