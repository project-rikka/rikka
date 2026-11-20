use crate::audio::AudioCodec;
use crate::encode::{EncodeSpeed, RateControl};

#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct AudioConfig {
    sample_rate: u32,
    channels: u16,
    codec: AudioCodec,
    rate: RateControl,
    bits_per_sample: Option<u32>,
    speed: EncodeSpeed,
}

impl AudioConfig {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
            codec: AudioCodec::Aac,
            rate: RateControl::Default,
            bits_per_sample: None,
            speed: EncodeSpeed::default(),
        }
    }

    pub fn codec(mut self, codec: AudioCodec) -> Self {
        self.codec = codec;
        self
    }

    pub fn rate(mut self, rate: RateControl) -> Self {
        self.rate = rate;
        self
    }

    pub fn bitrate(self, bits_per_second: u64) -> Self {
        self.rate(RateControl::Bitrate(bits_per_second))
    }

    pub fn quality(self, quality: u8) -> Self {
        self.rate(RateControl::Quality(quality))
    }

    pub fn bits_per_sample(mut self, bits: u32) -> Self {
        self.bits_per_sample = Some(bits);
        self
    }

    pub fn speed(mut self, speed: EncodeSpeed) -> Self {
        self.speed = speed;
        self
    }

    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    #[must_use]
    pub fn channels(&self) -> u16 {
        self.channels
    }

    #[must_use]
    pub fn encoded_as(&self) -> AudioCodec {
        self.codec
    }

    pub(crate) fn rate_control(&self) -> RateControl {
        self.rate
    }

    pub(crate) fn depth(&self) -> Option<u32> {
        self.bits_per_sample
    }

    pub(crate) fn pace(&self) -> EncodeSpeed {
        self.speed
    }
}
