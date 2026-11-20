mod filter_bank;
mod frame;

pub use frame::Frame;

pub const SAMPLE_RATE: u32 = 48_000;
pub const FRAME: usize = 480;
pub const BANDS: usize = 3;
pub const BAND: usize = 160;
pub const S16_SCALE: f32 = 32_768.0;
