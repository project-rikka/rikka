use discord_core::VideoCodec;

use crate::video::described::Described;
use crate::video::{av1, h264, h265, vp8, vp9};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Parsed {
    pub(crate) starts: Option<bool>,
    pub(crate) keyframe: bool,
}

impl From<Described> for Parsed {
    fn from(described: Described) -> Parsed {
        Parsed {
            starts: Some(described.starts),
            keyframe: described.keyframe,
        }
    }
}

pub(crate) fn parse(codec: VideoCodec, payload: &[u8]) -> Option<Parsed> {
    if payload.is_empty() {
        return None;
    }
    Some(match codec {
        VideoCodec::H264 => Parsed {
            starts: None,
            keyframe: h264::is_keyframe(payload),
        },
        VideoCodec::H265 => Parsed {
            starts: None,
            keyframe: h265::is_keyframe(payload),
        },
        VideoCodec::Vp8 => Parsed::from(vp8::describe(payload)?),
        VideoCodec::Vp9 => Parsed::from(vp9::describe(payload)?),
        VideoCodec::Av1 => Parsed {
            starts: Some(av1::starts(payload)),
            keyframe: av1::is_keyframe(payload),
        },
    })
}

pub(crate) fn assemble(codec: VideoCodec, payloads: &[&[u8]]) -> Option<Vec<u8>> {
    match codec {
        VideoCodec::H264 => h264::assemble(payloads),
        VideoCodec::H265 => h265::assemble(payloads),
        VideoCodec::Vp8 => vp8::assemble(payloads),
        VideoCodec::Vp9 => vp9::assemble(payloads),
        VideoCodec::Av1 => av1::assemble(payloads),
    }
}
