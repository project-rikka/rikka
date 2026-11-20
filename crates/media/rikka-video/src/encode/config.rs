use std::time::Duration;

use crate::audio::AudioConfig;
use crate::encode::backend::EncoderChoice;
use crate::encode::quality::{self, EncodeSpeed};
use crate::error::{Error, Result};

const FILE_KEYFRAMES: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VideoCodec {
    H264,
    Hevc,
    Av1,
    Vp9,
}

impl VideoCodec {
    pub(crate) const fn stream_name(self) -> &'static str {
        match self {
            VideoCodec::H264 => "h264",
            VideoCodec::Hevc => "hevc",
            VideoCodec::Av1 => "av1",
            VideoCodec::Vp9 => "vp9",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RateControl {
    #[default]
    Default,
    Bitrate(u64),
    Quality(u8),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Feed {
    #[default]
    Memory,
    #[cfg(unix)]
    Dmabuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyframeSpacing {
    Every(Duration),
    OnRequest,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ParameterSets {
    #[default]
    OutOfBand,
    InBand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Latency {
    Buffered,
    Immediate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Spec {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) frame_rate: (u32, u32),
    pub(crate) codec: VideoCodec,
    pub(crate) backend: EncoderChoice,
    pub(crate) rate: RateControl,
    pub(crate) speed: EncodeSpeed,
    pub(crate) keyframes: KeyframeSpacing,
    pub(crate) feed: Feed,
    pub(crate) parameter_sets: ParameterSets,
    pub(crate) latency: Latency,
}

impl Spec {
    pub(crate) fn checked(self) -> Result<Spec> {
        let (width, height) = (self.width, self.height);
        if width == 0 || height == 0 || !width.is_multiple_of(2) || !height.is_multiple_of(2) {
            return Err(Error::Unsupported(format!(
                "encode size {width}x{height} (must be non-zero and even)"
            )));
        }
        let (numerator, denominator) = self.frame_rate;
        if numerator == 0 || denominator == 0 || numerator / denominator > 240 {
            return Err(Error::Unsupported(format!(
                "{numerator}/{denominator} frames a second (must be above 0 and at most 240)"
            )));
        }
        if self.keyframes == KeyframeSpacing::Every(Duration::ZERO) {
            return Err(Error::Unsupported("keyframes every 0 seconds".into()));
        }
        if let RateControl::Quality(quality) = self.rate {
            quality::checked(quality)?;
        }
        Ok(self)
    }

    pub(crate) fn frame_interval(&self) -> Duration {
        let (numerator, denominator) = self.frame_rate;
        Duration::from_nanos(u64::from(denominator) * 1_000_000_000 / u64::from(numerator))
    }

    #[cfg(unix)]
    pub(crate) fn nominal_rate(&self) -> u32 {
        let (numerator, denominator) = self.frame_rate;
        numerator.div_ceil(denominator).max(1)
    }

    pub(crate) fn keyframe_period(&self) -> Option<i32> {
        match self.keyframes {
            KeyframeSpacing::Every(every) => {
                let (numerator, denominator) = self.frame_rate;
                let frames = every.as_secs_f64() * f64::from(numerator) / f64::from(denominator);
                Some((frames.round().min(f64::from(i32::MAX)) as i32).max(1))
            }
            KeyframeSpacing::OnRequest => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct EncoderConfig {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) frame_rate: (u32, u32),
    pub(crate) codec: VideoCodec,
    pub(crate) backend: EncoderChoice,
    pub(crate) rate: RateControl,
    pub(crate) speed: EncodeSpeed,
    pub(crate) feed: Feed,
    pub(crate) audio: Option<AudioConfig>,
    pub(crate) fast_start: bool,
}

impl EncoderConfig {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            frame_rate: (30, 1),
            codec: VideoCodec::H264,
            backend: EncoderChoice::Auto,
            rate: RateControl::Default,
            speed: EncodeSpeed::default(),
            feed: Feed::Memory,
            audio: None,
            fast_start: false,
        }
    }

    pub fn frame_rate(mut self, fps: u32) -> Self {
        self.frame_rate = (fps, 1);
        self
    }

    pub fn frame_rate_rational(mut self, numerator: u32, denominator: u32) -> Self {
        self.frame_rate = (numerator, denominator);
        self
    }

    pub fn codec(mut self, codec: VideoCodec) -> Self {
        self.codec = codec;
        self
    }

    pub fn backend(mut self, backend: impl Into<EncoderChoice>) -> Self {
        self.backend = backend.into();
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

    pub fn speed(mut self, speed: EncodeSpeed) -> Self {
        self.speed = speed;
        self
    }

    pub fn feed(mut self, feed: Feed) -> Self {
        self.feed = feed;
        self
    }

    pub fn audio(mut self, audio: AudioConfig) -> Self {
        self.audio = Some(audio);
        self
    }

    pub fn fast_start(mut self) -> Self {
        self.fast_start = true;
        self
    }

    pub(crate) fn spec(&self, parameter_sets: ParameterSets) -> Spec {
        Spec {
            width: self.width,
            height: self.height,
            frame_rate: self.frame_rate,
            codec: self.codec,
            backend: self.backend,
            rate: self.rate,
            speed: self.speed,
            keyframes: KeyframeSpacing::Every(FILE_KEYFRAMES),
            feed: self.feed,
            parameter_sets,
            latency: Latency::Buffered,
        }
    }
}
