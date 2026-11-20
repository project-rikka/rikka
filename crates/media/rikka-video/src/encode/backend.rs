use std::ffi::CString;

use crate::encode::config::VideoCodec;
use crate::encode::quality::Family;
use rikka_ffmpeg::avcodec::{self, AVCodec};

const VENDOR_NVIDIA: u32 = 0x10de;
const VENDOR_AMD: u32 = 0x1002;
const VENDOR_INTEL: u32 = 0x8086;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EncoderBackend {
    Nvenc,
    Amf,
    Qsv,
    Vaapi,
    VulkanVideo,
    Software,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EncoderChoice {
    #[default]
    Auto,
    Only(EncoderBackend),
}

impl From<EncoderBackend> for EncoderChoice {
    fn from(backend: EncoderBackend) -> Self {
        Self::Only(backend)
    }
}

impl EncoderChoice {
    pub(crate) fn candidates(self, vendor: Option<u32>) -> Vec<EncoderBackend> {
        match self {
            Self::Only(backend) => vec![backend],
            Self::Auto => {
                let mut order = EncoderBackend::hardware_order(vendor).to_vec();
                order.push(EncoderBackend::Software);
                order
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Encoder {
    pub(crate) name: &'static str,
    pub(crate) family: Option<Family>,
}
impl Encoder {
    const fn new(name: &'static str, family: Family) -> Self {
        Self {
            name,
            family: Some(family),
        }
    }

    const fn unclassified(name: &'static str) -> Self {
        Self { name, family: None }
    }
}

impl EncoderBackend {
    #[must_use]
    pub const fn is_hardware(self) -> bool {
        !matches!(self, Self::Software)
    }

    pub(crate) const fn encoders(self, codec: VideoCodec) -> &'static [Encoder] {
        use Family::*;
        match (self, codec) {
            (Self::Nvenc, VideoCodec::H264) => const { &[Encoder::new("h264_nvenc", Nvenc)] },
            (Self::Nvenc, VideoCodec::Hevc) => const { &[Encoder::new("hevc_nvenc", Nvenc)] },
            (Self::Nvenc, VideoCodec::Av1) => const { &[Encoder::new("av1_nvenc", Nvenc)] },
            (Self::Amf, VideoCodec::H264) => const { &[Encoder::new("h264_amf", Amf)] },
            (Self::Amf, VideoCodec::Hevc) => const { &[Encoder::new("hevc_amf", Amf)] },
            (Self::Amf, VideoCodec::Av1) => const { &[Encoder::new("av1_amf", Amf)] },
            (Self::Qsv, VideoCodec::H264) => const { &[Encoder::new("h264_qsv", Qsv)] },
            (Self::Qsv, VideoCodec::Hevc) => const { &[Encoder::new("hevc_qsv", Qsv)] },
            (Self::Qsv, VideoCodec::Av1) => const { &[Encoder::new("av1_qsv", Qsv)] },
            (Self::Vaapi, VideoCodec::H264) => const { &[Encoder::new("h264_vaapi", Vaapi)] },
            (Self::Vaapi, VideoCodec::Hevc) => const { &[Encoder::new("hevc_vaapi", Vaapi)] },
            (Self::Vaapi, VideoCodec::Av1) => const { &[Encoder::new("av1_vaapi", Vaapi)] },
            (Self::Vaapi, VideoCodec::Vp9) => const { &[Encoder::new("vp9_vaapi", Vaapi)] },
            (Self::Qsv, VideoCodec::Vp9) => const { &[Encoder::new("vp9_qsv", Qsv)] },
            (Self::VulkanVideo, _) | (Self::Nvenc | Self::Amf, VideoCodec::Vp9) => &[],
            (Self::Software, VideoCodec::Vp9) => const { &[Encoder::new("libvpx-vp9", Vpx)] },
            (Self::Software, VideoCodec::H264) => {
                const {
                    &[
                        Encoder::new("libx264", X26x),
                        Encoder::unclassified("libopenh264"),
                    ]
                }
            }
            (Self::Software, VideoCodec::Hevc) => {
                const {
                    &[
                        Encoder::new("libx265", X26x),
                        Encoder::new("libkvazaar", Kvazaar),
                    ]
                }
            }
            (Self::Software, VideoCodec::Av1) => {
                const {
                    &[
                        Encoder::new("libsvtav1", SvtAv1),
                        Encoder::new("librav1e", Rav1e),
                        Encoder::new("libaom-av1", Aom),
                    ]
                }
            }
        }
    }

    #[cfg(unix)]
    const fn hardware_order(vendor: Option<u32>) -> &'static [EncoderBackend] {
        match vendor {
            Some(VENDOR_NVIDIA) => &[Self::Nvenc],
            Some(VENDOR_AMD) => &[Self::Amf, Self::Vaapi],
            Some(VENDOR_INTEL) => &[Self::Qsv, Self::Vaapi],
            _ => &[Self::Nvenc, Self::Qsv, Self::Amf, Self::Vaapi],
        }
    }

    #[cfg(windows)]
    const fn hardware_order(vendor: Option<u32>) -> &'static [EncoderBackend] {
        match vendor {
            Some(VENDOR_NVIDIA) => &[Self::Nvenc],
            Some(VENDOR_AMD) => &[Self::Amf],
            Some(VENDOR_INTEL) => &[Self::Qsv],
            _ => &[Self::Nvenc, Self::Qsv, Self::Amf],
        }
    }
}

pub(crate) fn find_encoder(name: &str) -> Option<*const AVCodec> {
    let c_name = CString::new(name).ok()?;
    let codec = unsafe { avcodec::avcodec_find_encoder_by_name(c_name.as_ptr()) };
    (!codec.is_null()).then_some(codec)
}
