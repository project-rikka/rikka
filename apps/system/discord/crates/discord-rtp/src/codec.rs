use discord_core::VideoCodec;

pub const OPUS_PAYLOAD_TYPE: u8 = 120;

pub const SENT_CODEC: VideoCodec = VideoCodec::H264;

pub const VIDEO_CODECS: [VideoCodec; 5] = [
    VideoCodec::Av1,
    VideoCodec::H265,
    VideoCodec::H264,
    VideoCodec::Vp8,
    VideoCodec::Vp9,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Payload {
    Opus,
    Video(VideoCodec),
    Repair(VideoCodec),
}

impl Payload {
    #[must_use]
    pub fn from_type(payload_type: u8) -> Option<Payload> {
        if payload_type == OPUS_PAYLOAD_TYPE {
            return Some(Payload::Opus);
        }
        VIDEO_CODECS.into_iter().find_map(|codec| {
            let primary = primary(codec);
            if payload_type == primary {
                Some(Payload::Video(codec))
            } else if payload_type == primary + 1 {
                Some(Payload::Repair(codec))
            } else {
                None
            }
        })
    }

    #[must_use]
    pub fn payload_type(self) -> u8 {
        match self {
            Payload::Opus => OPUS_PAYLOAD_TYPE,
            Payload::Video(codec) => primary(codec),
            Payload::Repair(codec) => primary(codec) + 1,
        }
    }
}

fn primary(codec: VideoCodec) -> u8 {
    match codec {
        VideoCodec::Av1 => 109,
        VideoCodec::H265 => 103,
        VideoCodec::H264 => 101,
        VideoCodec::Vp8 => 105,
        VideoCodec::Vp9 => 107,
    }
}
