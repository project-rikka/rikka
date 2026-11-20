use rikka_remote_wire::Codec as WireCodec;
use rikka_session::remote::Codec as StatusCodec;
use rikka_vulkan_video::Codec;

pub(crate) fn from_wire(codec: WireCodec) -> Option<Codec> {
    match codec {
        WireCodec::H264 => Some(Codec::H264),
        WireCodec::H265 => Some(Codec::H265),
        WireCodec::Av1 => None,
    }
}

pub(crate) fn to_wire(codec: Codec) -> WireCodec {
    match codec {
        Codec::H264 => WireCodec::H264,
        Codec::H265 => WireCodec::H265,
    }
}

pub(crate) fn to_status(codec: Codec) -> StatusCodec {
    match codec {
        Codec::H264 => StatusCodec::H264,
        Codec::H265 => StatusCodec::H265,
    }
}
