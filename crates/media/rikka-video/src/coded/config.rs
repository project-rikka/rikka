use std::time::Duration;

use crate::encode::{
    EncodeSpeed, EncoderChoice, Feed, KeyframeSpacing, Latency, ParameterSets, RateControl, Spec,
    VideoCodec,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub struct PacketConfig {
    spec: Spec,
}

impl PacketConfig {
    pub fn new(width: u32, height: u32) -> PacketConfig {
        PacketConfig {
            spec: Spec {
                width,
                height,
                frame_rate: (60, 1),
                codec: VideoCodec::H264,
                backend: EncoderChoice::Auto,
                rate: RateControl::Quality(70),
                speed: EncodeSpeed::Fast,
                keyframes: KeyframeSpacing::Every(Duration::from_secs(2)),
                feed: Feed::Memory,
                parameter_sets: ParameterSets::OutOfBand,
                latency: Latency::Immediate,
            },
        }
    }

    pub fn frame_rate(self, frame_rate: u32) -> PacketConfig {
        self.with(|spec| spec.frame_rate = (frame_rate, 1))
    }

    pub fn codec(self, codec: VideoCodec) -> PacketConfig {
        self.with(|spec| spec.codec = codec)
    }

    pub fn backend(self, backend: impl Into<EncoderChoice>) -> PacketConfig {
        let backend = backend.into();
        self.with(|spec| spec.backend = backend)
    }

    pub fn rate(self, rate: RateControl) -> PacketConfig {
        self.with(|spec| spec.rate = rate)
    }

    pub fn quality(self, quality: u8) -> PacketConfig {
        self.rate(RateControl::Quality(quality))
    }

    pub fn bitrate(self, bits_per_second: u64) -> PacketConfig {
        self.rate(RateControl::Bitrate(bits_per_second))
    }

    pub fn speed(self, speed: EncodeSpeed) -> PacketConfig {
        self.with(|spec| spec.speed = speed)
    }

    pub fn keyframe_every(self, every: Duration) -> PacketConfig {
        self.with(|spec| spec.keyframes = KeyframeSpacing::Every(every))
    }

    pub fn keyframes_on_request(self) -> PacketConfig {
        self.with(|spec| spec.keyframes = KeyframeSpacing::OnRequest)
    }

    pub fn feed(self, feed: Feed) -> PacketConfig {
        self.with(|spec| spec.feed = feed)
    }

    pub fn parameter_sets(self, parameter_sets: ParameterSets) -> PacketConfig {
        self.with(|spec| spec.parameter_sets = parameter_sets)
    }

    pub(crate) fn spec(self) -> Spec {
        self.spec
    }

    fn with(mut self, change: impl FnOnce(&mut Spec)) -> PacketConfig {
        change(&mut self.spec);
        self
    }
}
