mod h264;
mod h265;
mod plain;

use std::ffi::c_void;

use ash::vk;

use crate::Codec;
use crate::capabilities::{Capabilities, CodecLimits};

pub(crate) trait Chain: Send {
    fn next(&self) -> *const c_void;
}

pub(crate) trait RateChain: Send {
    fn info(&self) -> *const c_void;
    fn layer(&self) -> *const c_void;
}

pub(crate) trait PictureChain: Send {
    fn info(&self) -> *const c_void;
    fn setup(&self) -> *const c_void;
    fn reference(&self) -> *const c_void;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    pub(crate) picture: vk::Extent2D,
    pub(crate) coded: vk::Extent2D,
    pub(crate) references: u8,
    pub(crate) frame_rate: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Reference {
    pub(crate) slot: u8,
    pub(crate) order: u32,
    pub(crate) intra: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Plan {
    pub(crate) intra: bool,
    pub(crate) order: u32,
    pub(crate) idr_id: u16,
    pub(crate) setup_slot: u8,
    pub(crate) reference: Option<Reference>,
    pub(crate) kept: Vec<Reference>,
    pub(crate) qp: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RateSettings {
    pub(crate) min_qp: i32,
    pub(crate) max_qp: i32,
    pub(crate) max_intra_bytes: u32,
    pub(crate) max_predicted_bytes: u32,
}

#[derive(Debug)]
pub(crate) enum Syntax {
    H264(h264::Syntax),
    H265(h265::Syntax),
}

impl Syntax {
    pub(crate) fn coded(
        codec: Codec,
        width: u32,
        height: u32,
        capabilities: &Capabilities,
    ) -> vk::Extent2D {
        match codec {
            Codec::H264 => h264::coded(width, height),
            Codec::H265 => h265::coded(width, height, capabilities.granularity),
        }
    }

    pub(crate) fn most_references(codec: Codec) -> u8 {
        match codec {
            Codec::H264 => h264::most_references(),
            Codec::H265 => h265::most_references(),
        }
    }

    pub(crate) fn new(layout: Layout, capabilities: &Capabilities) -> Syntax {
        match capabilities.codec {
            CodecLimits::H264(limits) => Syntax::H264(h264::Syntax::new(layout, limits.max_level)),
            CodecLimits::H265(limits) => Syntax::H265(h265::Syntax::new(
                layout,
                limits.max_level,
                h265::Blocks::choose(limits.ctb_sizes, limits.transform_sizes),
                limits.syntax,
            )),
        }
    }

    pub(crate) fn parameters(&self) -> Box<dyn Chain> {
        match self {
            Syntax::H264(syntax) => syntax.parameters(),
            Syntax::H265(syntax) => syntax.parameters(),
        }
    }

    pub(crate) fn header_query(&self) -> Box<dyn Chain> {
        match self {
            Syntax::H264(syntax) => syntax.header_query(),
            Syntax::H265(syntax) => syntax.header_query(),
        }
    }

    pub(crate) fn rate(&self, rate: &RateSettings) -> Box<dyn RateChain> {
        match self {
            Syntax::H264(syntax) => syntax.rate(rate),
            Syntax::H265(syntax) => syntax.rate(rate),
        }
    }

    pub(crate) fn picture(&self, plan: &Plan) -> Box<dyn PictureChain> {
        match self {
            Syntax::H264(syntax) => syntax.picture(plan),
            Syntax::H265(syntax) => syntax.picture(plan),
        }
    }
}
