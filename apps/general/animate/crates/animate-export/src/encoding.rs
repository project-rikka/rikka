use rikka_video::{EncodeSpeed, EncoderBackend, EncoderChoice, VideoCodec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRate {
    numerator: u32,
    denominator: u32,
}

impl FrameRate {
    const fn whole(per_second: u32) -> Self {
        Self {
            numerator: per_second,
            denominator: 1,
        }
    }

    const fn ntsc(per_second: u32) -> Self {
        Self {
            numerator: per_second * 1000,
            denominator: 1001,
        }
    }

    pub fn numerator(self) -> u32 {
        self.numerator
    }

    pub fn denominator(self) -> u32 {
        self.denominator
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Mp4,
    Mkv,
    Mov,
}

impl Container {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mkv => "mkv",
            Self::Mov => "mov",
        }
    }

    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Mp4 => "video/mp4",
            Self::Mkv => "video/x-matroska",
            Self::Mov => "video/quicktime",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rate {
    Quality,
    Bitrate,
}

pub const FRAME_RATES: [(FrameRate, &str); 8] = [
    (FrameRate::ntsc(24), "23.976"),
    (FrameRate::whole(24), "24"),
    (FrameRate::whole(25), "25"),
    (FrameRate::ntsc(30), "29.97"),
    (FrameRate::whole(30), "30"),
    (FrameRate::whole(50), "50"),
    (FrameRate::ntsc(60), "59.94"),
    (FrameRate::whole(60), "60"),
];

pub const CONTAINERS: [(Container, &str); 3] = [
    (Container::Mp4, "mp4"),
    (Container::Mkv, "mkv"),
    (Container::Mov, "mov"),
];

pub const CODECS: [(VideoCodec, &str); 3] = [
    (VideoCodec::H264, "h264"),
    (VideoCodec::Hevc, "hevc"),
    (VideoCodec::Av1, "av1"),
];

pub const ENCODERS: [(EncoderChoice, &str); 6] = [
    (EncoderChoice::Auto, "auto"),
    (EncoderChoice::Only(EncoderBackend::Nvenc), "nvenc"),
    (EncoderChoice::Only(EncoderBackend::Amf), "amf"),
    (EncoderChoice::Only(EncoderBackend::Qsv), "qsv"),
    (EncoderChoice::Only(EncoderBackend::Vaapi), "vaapi"),
    (EncoderChoice::Only(EncoderBackend::Software), "software"),
];

pub const SPEEDS: [(EncodeSpeed, &str); 4] = [
    (EncodeSpeed::Fastest, "fastest"),
    (EncodeSpeed::Fast, "fast"),
    (EncodeSpeed::Balanced, "balanced"),
    (EncodeSpeed::Slow, "slow"),
];

pub const RATES: [(Rate, &str); 2] = [(Rate::Quality, "quality"), (Rate::Bitrate, "bitrate")];

pub const SOUND_BITRATES: [u32; 4] = [128, 192, 256, 320];

pub const QUALITIES: std::ops::RangeInclusive<u8> = 1..=100;
pub const BITRATES: std::ops::RangeInclusive<u32> = 1..=500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Encoding {
    pub frame_rate: FrameRate,
    pub container: Container,
    pub codec: VideoCodec,
    pub encoder: EncoderChoice,
    pub speed: EncodeSpeed,
    pub rate: Rate,
    pub quality: u8,
    pub bitrate: u32,
    pub sound: bool,
    pub sound_bitrate: u32,
}

impl Default for Encoding {
    fn default() -> Self {
        Self {
            frame_rate: FrameRate::whole(30),
            container: Container::Mp4,
            codec: VideoCodec::H264,
            encoder: EncoderChoice::Auto,
            speed: EncodeSpeed::Balanced,
            rate: Rate::Quality,
            quality: 80,
            bitrate: 20,
            sound: true,
            sound_bitrate: 256,
        }
    }
}

pub fn position<T: Copy + PartialEq>(table: &[(T, &str)], value: T) -> Option<usize> {
    table.iter().position(|(candidate, _)| *candidate == value)
}

pub fn name_of<T: Copy + PartialEq>(table: &[(T, &'static str)], value: T) -> &'static str {
    position(table, value).map_or(table[0].1, |at| table[at].1)
}

pub fn named<T: Copy>(table: &[(T, &str)], name: &str) -> Option<T> {
    table
        .iter()
        .find(|(_, candidate)| *candidate == name)
        .map(|(value, _)| *value)
}
