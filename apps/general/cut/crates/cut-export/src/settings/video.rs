use std::ops::RangeInclusive;

use cut_project::Format;
use rikka_video::{EncodeSpeed, EncoderBackend, EncoderChoice, VideoCodec};

use super::picture::{FrameRate, Resolution};

const SMALL_SIDE: u32 = 720;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Container {
    #[default]
    Mp4,
    Mov,
    Mkv,
}

impl Container {
    pub const ALL: [Container; 3] = [Container::Mp4, Container::Mov, Container::Mkv];

    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::Mov => "mov",
            Container::Mkv => "mkv",
        }
    }

    #[must_use]
    pub fn mime_type(self) -> &'static str {
        match self {
            Container::Mp4 => "video/mp4",
            Container::Mov => "video/quicktime",
            Container::Mkv => "video/x-matroska",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Compression {
    #[default]
    Quality,
    Bitrate,
}

impl Compression {
    pub const ALL: [Compression; 2] = [Compression::Quality, Compression::Bitrate];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Preset {
    High,
    Standard,
    Small,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::High, Preset::Standard, Preset::Small];

    fn quality(self) -> u8 {
        match self {
            Preset::High => 90,
            Preset::Standard => 75,
            Preset::Small => 60,
        }
    }

    fn sound_bitrate(self) -> u32 {
        match self {
            Preset::High => 320,
            Preset::Standard => 256,
            Preset::Small => 128,
        }
    }

    fn resolution(self, format: Format) -> Resolution {
        match self {
            Preset::Small if format.width.min(format.height) > SMALL_SIDE => {
                Resolution::Short(SMALL_SIDE)
            }
            Preset::High | Preset::Standard | Preset::Small => Resolution::Project,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Video {
    pub container: Container,
    pub codec: VideoCodec,
    pub encoder: EncoderChoice,
    pub speed: EncodeSpeed,
    pub compression: Compression,
    pub quality: u8,
    pub bitrate: u32,
    pub resolution: Resolution,
    pub frame_rate: FrameRate,
    pub sound: bool,
    pub sound_bitrate: u32,
}

impl Video {
    pub const QUALITIES: RangeInclusive<u8> = 1..=100;
    pub const BITRATES: RangeInclusive<u32> = 1..=500;
    pub const CODECS: [VideoCodec; 3] = [VideoCodec::H264, VideoCodec::Hevc, VideoCodec::Av1];
    pub const ENCODERS: [EncoderChoice; 6] = [
        EncoderChoice::Auto,
        EncoderChoice::Only(EncoderBackend::Nvenc),
        EncoderChoice::Only(EncoderBackend::Amf),
        EncoderChoice::Only(EncoderBackend::Qsv),
        EncoderChoice::Only(EncoderBackend::Vaapi),
        EncoderChoice::Only(EncoderBackend::Software),
    ];
    pub const SPEEDS: [EncodeSpeed; 4] = [
        EncodeSpeed::Fastest,
        EncodeSpeed::Fast,
        EncodeSpeed::Balanced,
        EncodeSpeed::Slow,
    ];

    #[must_use]
    pub fn with_preset(self, preset: Preset, format: Format) -> Video {
        Video {
            container: Container::Mp4,
            codec: VideoCodec::H264,
            compression: Compression::Quality,
            quality: preset.quality(),
            resolution: preset.resolution(format),
            frame_rate: FrameRate::Project,
            sound: true,
            sound_bitrate: preset.sound_bitrate(),
            ..self
        }
    }

    #[must_use]
    pub fn preset(&self, format: Format) -> Option<Preset> {
        Preset::ALL
            .into_iter()
            .find(|preset| self.with_preset(*preset, format) == *self)
    }
}

impl Default for Video {
    fn default() -> Video {
        Video {
            container: Container::Mp4,
            codec: VideoCodec::H264,
            encoder: EncoderChoice::Auto,
            speed: EncodeSpeed::Balanced,
            compression: Compression::Quality,
            quality: Preset::Standard.quality(),
            bitrate: 20,
            resolution: Resolution::Project,
            frame_rate: FrameRate::Project,
            sound: true,
            sound_bitrate: Preset::Standard.sound_bitrate(),
        }
    }
}
