use std::time::Duration;

pub const VIDEO_CLOCK: u32 = 90_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VideoCodec {
    Av1,
    H265,
    H264,
    Vp8,
    Vp9,
}

impl VideoCodec {
    pub const ALL: [VideoCodec; 5] = [
        VideoCodec::Av1,
        VideoCodec::H265,
        VideoCodec::H264,
        VideoCodec::Vp8,
        VideoCodec::Vp9,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            VideoCodec::Av1 => "AV1",
            VideoCodec::H265 => "H265",
            VideoCodec::H264 => "H264",
            VideoCodec::Vp8 => "VP8",
            VideoCodec::Vp9 => "VP9",
        }
    }

    #[must_use]
    pub fn named(name: &str) -> Option<VideoCodec> {
        VideoCodec::ALL
            .into_iter()
            .find(|codec| codec.name().eq_ignore_ascii_case(name))
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct VideoFrame {
    pub source: u32,
    pub codec: VideoCodec,
    pub timestamp: u32,
    pub keyframe: bool,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for VideoFrame {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VideoFrame")
            .field("source", &self.source)
            .field("codec", &self.codec)
            .field("timestamp", &self.timestamp)
            .field("keyframe", &self.keyframe)
            .field("bytes", &self.data.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct EncodedVideo {
    pub data: Vec<u8>,
    pub keyframe: bool,
    pub captured: Duration,
}

impl std::fmt::Debug for EncodedVideo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EncodedVideo")
            .field("keyframe", &self.keyframe)
            .field("captured", &self.captured)
            .field("bytes", &self.data.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoShape {
    pub width: u32,
    pub height: u32,
    pub frame_rate: u32,
    pub bitrate: u32,
}
