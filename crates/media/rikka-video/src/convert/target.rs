use rikka_types::PixelSize;

use crate::audio::AudioCodec;
use crate::encode::{EncodeSpeed, RateControl, VideoCodec};

const STANDARD_QUALITY: u8 = 65;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub struct VideoTarget {
    pub(crate) codec: VideoCodec,
    pub(crate) rate: RateControl,
    pub(crate) speed: EncodeSpeed,
    pub(crate) bounds: Option<(u32, u32)>,
    pub(crate) reencode: bool,
}

impl VideoTarget {
    pub fn new(codec: VideoCodec) -> Self {
        Self {
            codec,
            rate: RateControl::Quality(STANDARD_QUALITY),
            speed: EncodeSpeed::Balanced,
            bounds: None,
            reencode: false,
        }
    }

    pub fn quality(mut self, quality: u8) -> Self {
        self.rate = RateControl::Quality(quality.clamp(1, 100));
        self
    }

    pub fn bitrate(mut self, bits_per_second: u64) -> Self {
        self.rate = RateControl::Bitrate(bits_per_second);
        self
    }

    pub fn speed(mut self, speed: EncodeSpeed) -> Self {
        self.speed = speed;
        self
    }

    pub fn with_max_size(mut self, size: PixelSize) -> Self {
        self.bounds = Some((size.width.max(2), size.height.max(2)));
        self
    }

    pub fn reencode(mut self) -> Self {
        self.reencode = true;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub struct AudioTarget {
    pub(crate) codec: AudioCodec,
    pub(crate) rate: RateControl,
    pub(crate) speed: EncodeSpeed,
    pub(crate) reencode: bool,
}

impl AudioTarget {
    pub fn new(codec: AudioCodec) -> Self {
        Self {
            codec,
            rate: RateControl::Default,
            speed: EncodeSpeed::Balanced,
            reencode: false,
        }
    }

    pub fn quality(mut self, quality: u8) -> Self {
        self.rate = RateControl::Quality(quality.clamp(1, 100));
        self
    }

    pub fn bitrate(mut self, bits_per_second: u64) -> Self {
        self.rate = RateControl::Bitrate(bits_per_second);
        self
    }

    pub fn speed(mut self, speed: EncodeSpeed) -> Self {
        self.speed = speed;
        self
    }

    pub fn reencode(mut self) -> Self {
        self.reencode = true;
        self
    }
}
