#![allow(
    missing_debug_implementations,
    non_camel_case_types,
    non_snake_case,
    dead_code,
    clippy::upper_case_acronyms
)]

use core::ffi::{c_char, c_int, c_uint, c_void};

use crate::symbols;

#[repr(C)]
pub struct AVBuffer {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct AVBufferPool {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct AVClass {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct AVDictionary {
    _opaque: [u8; 0],
}

pub type AVFrameSideDataType = c_int;
pub const AV_FRAME_DATA_ICC_PROFILE: AVFrameSideDataType = 15;

#[repr(C)]
pub struct AVFrameSideData {
    pub r#type: AVFrameSideDataType,
    pub data: *mut u8,
    pub size: usize,
    pub metadata: *mut AVDictionary,
    pub buf: *mut AVBufferRef,
}

#[repr(C)]
pub struct AVChannelCustom {
    _opaque: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct AVRational {
    pub num: c_int,
    pub den: c_int,
}

#[repr(C)]
pub struct AVBufferRef {
    pub buffer: *mut AVBuffer,
    pub data: *mut u8,
    pub size: usize,
}

#[repr(C)]
pub struct AVDictionaryEntry {
    pub key: *mut c_char,
    pub value: *mut c_char,
}

pub type AVChannelOrder = c_int;
pub const AV_CHANNEL_ORDER_UNSPEC: AVChannelOrder = 0;
pub const AV_CHANNEL_ORDER_NATIVE: AVChannelOrder = 1;
pub const AV_CHANNEL_ORDER_CUSTOM: AVChannelOrder = 2;
pub const AV_CHANNEL_ORDER_AMBISONIC: AVChannelOrder = 3;

#[repr(C)]
pub union AVChannelLayout_union {
    pub mask: u64,
    pub map: *mut AVChannelCustom,
}

#[repr(C)]
pub struct AVChannelLayout {
    pub order: AVChannelOrder,
    pub nb_channels: c_int,
    pub u: AVChannelLayout_union,
    pub opaque: *mut c_void,
}

pub type AVMediaType = c_int;
pub const AVMEDIA_TYPE_UNKNOWN: AVMediaType = -1;
pub const AVMEDIA_TYPE_VIDEO: AVMediaType = 0;
pub const AVMEDIA_TYPE_AUDIO: AVMediaType = 1;
pub const AVMEDIA_TYPE_DATA: AVMediaType = 2;
pub const AVMEDIA_TYPE_SUBTITLE: AVMediaType = 3;
pub const AVMEDIA_TYPE_ATTACHMENT: AVMediaType = 4;

pub type AVPictureType = c_int;
pub const AV_PICTURE_TYPE_NONE: AVPictureType = 0;
pub const AV_PICTURE_TYPE_I: AVPictureType = 1;

pub const AV_NOPTS_VALUE: i64 = i64::MIN;
pub const AV_TIME_BASE: c_int = 1_000_000;
pub const AV_TIME_BASE_Q: AVRational = AVRational {
    num: 1,
    den: AV_TIME_BASE,
};

pub type AVPixelFormat = c_int;
pub const AV_PIX_FMT_NONE: AVPixelFormat = -1;
pub const AV_PIX_FMT_YUV420P: AVPixelFormat = 0;
pub const AV_PIX_FMT_RGB24: AVPixelFormat = 2;
pub const AV_PIX_FMT_YUV422P: AVPixelFormat = 4;
pub const AV_PIX_FMT_YUV444P: AVPixelFormat = 5;
pub const AV_PIX_FMT_GRAY8: AVPixelFormat = 8;
pub const AV_PIX_FMT_YUVJ420P: AVPixelFormat = 12;
pub const AV_PIX_FMT_YUVJ422P: AVPixelFormat = 13;
pub const AV_PIX_FMT_YUVJ444P: AVPixelFormat = 14;
pub const AV_PIX_FMT_NV12: AVPixelFormat = 23;
pub const AV_PIX_FMT_RGBA: AVPixelFormat = 26;
pub const AV_PIX_FMT_BGRA: AVPixelFormat = 28;
pub const AV_PIX_FMT_YUV420P10LE: AVPixelFormat = 62;
pub const AV_PIX_FMT_P010LE: AVPixelFormat = 158;
pub const AV_PIX_FMT_P016LE: AVPixelFormat = 169;
pub const AV_PIX_FMT_VAAPI: AVPixelFormat = 44;
pub const AV_PIX_FMT_VULKAN: AVPixelFormat = 190;
pub const AV_PIX_FMT_GRAY16LE: AVPixelFormat = 30;
pub const AV_PIX_FMT_RGB48LE: AVPixelFormat = 35;
pub const AV_PIX_FMT_YA8: AVPixelFormat = 56;
pub const AV_PIX_FMT_YUV444P10LE: AVPixelFormat = 68;
pub const AV_PIX_FMT_RGBA64LE: AVPixelFormat = 105;
pub const AV_PIX_FMT_YA16LE: AVPixelFormat = 110;
pub const AV_PIX_FMT_GRAY10LE: AVPixelFormat = 168;
pub const AV_PIX_FMT_GBRPF32BE: AVPixelFormat = 174;
pub const AV_PIX_FMT_GBRPF32LE: AVPixelFormat = 175;
pub const AV_PIX_FMT_GBRAPF32BE: AVPixelFormat = 176;
pub const AV_PIX_FMT_GBRAPF32LE: AVPixelFormat = 177;
pub const AV_PIX_FMT_GRAYF32LE: AVPixelFormat = 183;
pub const AV_PIX_FMT_RGBF32BE: AVPixelFormat = 217;
pub const AV_PIX_FMT_RGBF32LE: AVPixelFormat = 218;
pub const AV_PIX_FMT_RGBAF32BE: AVPixelFormat = 219;
pub const AV_PIX_FMT_RGBAF32LE: AVPixelFormat = 220;
pub const AV_PIX_FMT_GBRPF16LE: AVPixelFormat = 244;
pub const AV_PIX_FMT_GBRAPF16LE: AVPixelFormat = 246;
pub const AV_PIX_FMT_GRAYF16LE: AVPixelFormat = 248;
pub const AV_PIX_FMT_YAF32LE: AVPixelFormat = 253;
pub const AV_PIX_FMT_YAF16LE: AVPixelFormat = 255;

pub type AVColorPrimaries = c_int;
pub const AVCOL_PRI_BT709: AVColorPrimaries = 1;
pub const AVCOL_PRI_UNSPECIFIED: AVColorPrimaries = 2;
pub type AVColorTransferCharacteristic = c_int;
pub const AVCOL_TRC_BT709: AVColorTransferCharacteristic = 1;
pub const AVCOL_TRC_UNSPECIFIED: AVColorTransferCharacteristic = 2;
pub const AVCOL_TRC_LINEAR: AVColorTransferCharacteristic = 8;
pub const AVCOL_TRC_IEC61966_2_1: AVColorTransferCharacteristic = 13;

pub type AVColorSpace = c_int;
pub const AVCOL_SPC_RGB: AVColorSpace = 0;
pub const AVCOL_SPC_BT709: AVColorSpace = 1;
pub const AVCOL_SPC_UNSPECIFIED: AVColorSpace = 2;
pub const AVCOL_SPC_BT470BG: AVColorSpace = 5;
pub const AVCOL_SPC_SMPTE170M: AVColorSpace = 6;
pub const AVCOL_SPC_SMPTE240M: AVColorSpace = 7;
pub const AVCOL_SPC_BT2020_NCL: AVColorSpace = 9;
pub const AVCOL_SPC_BT2020_CL: AVColorSpace = 10;

pub type AVColorRange = c_int;
pub const AVCOL_RANGE_UNSPECIFIED: AVColorRange = 0;
pub const AVCOL_RANGE_MPEG: AVColorRange = 1;
pub const AVCOL_RANGE_JPEG: AVColorRange = 2;

pub type AVChromaLocation = c_int;
pub const AVCHROMA_LOC_UNSPECIFIED: AVChromaLocation = 0;
pub const AVCHROMA_LOC_LEFT: AVChromaLocation = 1;
pub const AVCHROMA_LOC_CENTER: AVChromaLocation = 2;
pub const AVCHROMA_LOC_TOPLEFT: AVChromaLocation = 3;

pub type AVAlphaMode = c_int;

pub const AV_NUM_DATA_POINTERS: usize = 8;

pub const AV_FRAME_FLAG_CORRUPT: c_int = 1 << 0;
pub const AV_FRAME_FLAG_KEY: c_int = 1 << 1;
pub const AV_FRAME_FLAG_INTERLACED: c_int = 1 << 3;

#[repr(C)]
pub struct AVFrame {
    pub data: [*mut u8; AV_NUM_DATA_POINTERS],
    pub linesize: [c_int; AV_NUM_DATA_POINTERS],
    pub extended_data: *mut *mut u8,
    pub width: c_int,
    pub height: c_int,
    pub nb_samples: c_int,
    pub format: c_int,
    pub pict_type: AVPictureType,
    pub sample_aspect_ratio: AVRational,
    pub pts: i64,
    pub pkt_dts: i64,
    pub time_base: AVRational,
    pub quality: c_int,
    pub opaque: *mut c_void,
    pub repeat_pict: c_int,
    pub sample_rate: c_int,
    pub buf: [*mut AVBufferRef; AV_NUM_DATA_POINTERS],
    pub extended_buf: *mut *mut AVBufferRef,
    pub nb_extended_buf: c_int,
    pub side_data: *mut *mut AVFrameSideData,
    pub nb_side_data: c_int,
    pub flags: c_int,
    pub color_range: AVColorRange,
    pub color_primaries: AVColorPrimaries,
    pub color_trc: AVColorTransferCharacteristic,
    pub colorspace: AVColorSpace,
    pub chroma_location: AVChromaLocation,
    pub best_effort_timestamp: i64,
    pub metadata: *mut AVDictionary,
    pub decode_error_flags: c_int,
    pub hw_frames_ctx: *mut AVBufferRef,
    pub opaque_ref: *mut AVBufferRef,
    pub crop_top: usize,
    pub crop_bottom: usize,
    pub crop_left: usize,
    pub crop_right: usize,
    pub private_ref: *mut c_void,
    pub ch_layout: AVChannelLayout,
    pub duration: i64,
    pub alpha_mode: AVAlphaMode,
}

pub type AVHWDeviceType = c_int;
pub const AV_HWDEVICE_TYPE_NONE: AVHWDeviceType = 0;
pub const AV_HWDEVICE_TYPE_VAAPI: AVHWDeviceType = 3;
pub const AV_HWDEVICE_TYPE_VULKAN: AVHWDeviceType = 11;

pub type AVHWFrameTransferDirection = c_int;
pub const AV_HWFRAME_TRANSFER_DIRECTION_FROM: AVHWFrameTransferDirection = 0;
pub const AV_HWFRAME_TRANSFER_DIRECTION_TO: AVHWFrameTransferDirection = 1;

#[repr(C)]
pub struct AVHWDeviceContext {
    pub av_class: *const AVClass,
    pub r#type: AVHWDeviceType,
    pub hwctx: *mut c_void,
    pub free: Option<unsafe extern "C" fn(ctx: *mut AVHWDeviceContext)>,
    pub user_opaque: *mut c_void,
}

#[repr(C)]
pub struct AVHWFramesContext {
    pub av_class: *const AVClass,
    pub device_ref: *mut AVBufferRef,
    pub device_ctx: *mut AVHWDeviceContext,
    pub hwctx: *mut c_void,
    pub free: Option<unsafe extern "C" fn(ctx: *mut AVHWFramesContext)>,
    pub user_opaque: *mut c_void,
    pub pool: *mut AVBufferPool,
    pub initial_pool_size: c_int,
    pub format: AVPixelFormat,
    pub sw_format: AVPixelFormat,
    pub width: c_int,
    pub height: c_int,
}

pub const AVERROR_EOF: c_int = -541_478_725;
pub const AVERROR_DECODER_NOT_FOUND: c_int = -1_128_613_112;
pub const AVERROR_ENCODER_NOT_FOUND: c_int = -1_129_203_192;
pub const AVERROR_INVALIDDATA: c_int = -1_094_995_529;
pub const AVERROR_UNKNOWN: c_int = -1_313_558_101;
pub const AVERROR_EAGAIN: c_int = -11;
pub const AVERROR_ENOMEM: c_int = -12;
pub const AVERROR_EINVAL: c_int = -22;
pub const AVERROR_EIO: c_int = -5;
pub const AV_ERROR_MAX_STRING_SIZE: usize = 64;

pub const AV_LOG_QUIET: c_int = -8;
pub const AV_LOG_ERROR: c_int = 16;
pub const AV_LOG_WARNING: c_int = 24;
pub const AV_LOG_INFO: c_int = 32;
pub const AV_LOG_VERBOSE: c_int = 40;
pub const AV_LOG_DEBUG: c_int = 48;

pub const AV_OPT_SEARCH_CHILDREN: c_int = 1 << 0;

pub type AVRounding = c_int;
pub const AV_ROUND_ZERO: AVRounding = 0;
pub const AV_ROUND_INF: AVRounding = 1;
pub const AV_ROUND_DOWN: AVRounding = 2;
pub const AV_ROUND_UP: AVRounding = 3;
pub const AV_ROUND_NEAR_INF: AVRounding = 5;
pub const AV_ROUND_PASS_MINMAX: AVRounding = 8192;

symbols::symbols! {
    symbols::AVUTIL;

    fn avutil_version() -> c_uint;
    fn av_strerror(errnum: c_int, errbuf: *mut c_char, errbuf_size: usize) -> c_int;

    fn av_malloc(size: usize) -> *mut c_void;
    fn av_free(ptr: *mut c_void);
    fn av_freep(ptr: *mut c_void);

    fn av_frame_alloc() -> *mut AVFrame;
    fn av_frame_free(frame: *mut *mut AVFrame);
    fn av_frame_ref(dst: *mut AVFrame, src: *const AVFrame) -> c_int;
    fn av_frame_unref(frame: *mut AVFrame);
    fn av_frame_clone(src: *const AVFrame) -> *mut AVFrame;
    fn av_frame_get_buffer(frame: *mut AVFrame, align: c_int) -> c_int;
    fn av_frame_new_side_data(
        frame: *mut AVFrame,
        r#type: AVFrameSideDataType,
        size: usize,
    ) -> *mut AVFrameSideData;
    fn av_frame_get_side_data(
        frame: *const AVFrame,
        r#type: AVFrameSideDataType,
    ) -> *mut AVFrameSideData;

    fn av_buffer_ref(buf: *const AVBufferRef) -> *mut AVBufferRef;
    fn av_buffer_unref(buf: *mut *mut AVBufferRef);

    fn av_dict_set(
        pm: *mut *mut AVDictionary,
        key: *const c_char,
        value: *const c_char,
        flags: c_int,
    ) -> c_int;
    fn av_dict_get(
        m: *const AVDictionary,
        key: *const c_char,
        prev: *const AVDictionaryEntry,
        flags: c_int,
    ) -> *mut AVDictionaryEntry;
    fn av_dict_free(m: *mut *mut AVDictionary);
    fn av_dict_copy(
        dst: *mut *mut AVDictionary,
        src: *const AVDictionary,
        flags: c_int,
    ) -> c_int;

    fn av_hwdevice_ctx_alloc(r#type: AVHWDeviceType) -> *mut AVBufferRef;
    fn av_hwdevice_ctx_create(
        device_ctx: *mut *mut AVBufferRef,
        r#type: AVHWDeviceType,
        device: *const c_char,
        opts: *mut AVDictionary,
        flags: c_int,
    ) -> c_int;
    fn av_hwdevice_ctx_init(r#ref: *mut AVBufferRef) -> c_int;
    fn av_hwframe_ctx_alloc(device_ctx: *mut AVBufferRef) -> *mut AVBufferRef;
    fn av_hwframe_ctx_init(r#ref: *mut AVBufferRef) -> c_int;
    fn av_hwframe_get_buffer(
        hwframe_ctx: *mut AVBufferRef,
        frame: *mut AVFrame,
        flags: c_int,
    ) -> c_int;
    fn av_hwframe_transfer_data(
        dst: *mut AVFrame,
        src: *const AVFrame,
        flags: c_int,
    ) -> c_int;

    fn av_opt_set(
        obj: *mut c_void,
        name: *const c_char,
        val: *const c_char,
        search_flags: c_int,
    ) -> c_int;
    fn av_opt_set_int(
        obj: *mut c_void,
        name: *const c_char,
        val: i64,
        search_flags: c_int,
    ) -> c_int;
    fn av_opt_get_int(
        obj: *mut c_void,
        name: *const c_char,
        search_flags: c_int,
        out_val: *mut i64,
    ) -> c_int;

    fn av_log_set_level(level: c_int);

    fn av_rescale_q(a: i64, bq: AVRational, cq: AVRational) -> i64;
    fn av_rescale_rnd(a: i64, b: i64, c: i64, rnd: AVRounding) -> i64;

    fn av_pix_fmt_count_planes(pix_fmt: AVPixelFormat) -> c_int;
    fn av_get_pix_fmt_name(pix_fmt: AVPixelFormat) -> *const c_char;

    fn av_channel_layout_default(ch_layout: *mut AVChannelLayout, nb_channels: c_int);
    fn av_channel_layout_uninit(channel_layout: *mut AVChannelLayout);
}
