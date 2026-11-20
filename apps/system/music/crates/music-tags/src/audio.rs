use std::time::Duration;

use music_core::Codec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Audio {
    pub codec: Codec,
    pub lossless: bool,
    pub duration: Duration,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub channels: Option<u8>,
    pub bitrate: Option<u32>,
}
