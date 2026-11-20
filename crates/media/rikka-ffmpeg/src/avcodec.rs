#![allow(
    missing_debug_implementations,
    non_camel_case_types,
    non_snake_case,
    dead_code,
    clippy::upper_case_acronyms,
    clippy::type_complexity
)]
use crate::avutil::*;
use core::ffi::{c_char, c_int, c_uint, c_void};

use crate::symbols;

#[repr(C)]
pub struct AVCodecInternal {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct AVHWAccel {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct AVCodecDescriptor {
    pub id: AVCodecID,
    pub r#type: AVMediaType,
    pub name: *const c_char,
    pub long_name: *const c_char,
    pub props: c_int,
}

pub const AV_CODEC_PROP_INTRA_ONLY: c_int = 1 << 0;
pub const AV_CODEC_PROP_LOSSY: c_int = 1 << 1;
pub const AV_CODEC_PROP_LOSSLESS: c_int = 1 << 2;

pub type AVCodecID = c_int;
pub const AV_CODEC_ID_NONE: i32 = 0;
pub const AV_CODEC_ID_H264: i32 = 27;
pub const AV_CODEC_ID_VP8: i32 = 139;
pub const AV_CODEC_ID_VP9: i32 = 166;
pub const AV_CODEC_ID_HEVC: i32 = 172;
pub const AV_CODEC_ID_AV1: i32 = 222;
pub const AV_CODEC_ID_JPEG2000: i32 = 88;
pub const AV_CODEC_ID_TIFF: i32 = 96;
pub const AV_CODEC_ID_EXR: i32 = 177;

pub type AVFieldOrder = c_int;

pub type AVAudioServiceType = c_int;

pub type AVSampleFormat = c_int;
pub const AV_SAMPLE_FMT_NONE: i32 = -1;
pub const AV_SAMPLE_FMT_U8: i32 = 0;
pub const AV_SAMPLE_FMT_S16: i32 = 1;
pub const AV_SAMPLE_FMT_S32: i32 = 2;
pub const AV_SAMPLE_FMT_FLT: i32 = 3;
pub const AV_SAMPLE_FMT_DBL: i32 = 4;
pub const AV_SAMPLE_FMT_U8P: i32 = 5;
pub const AV_SAMPLE_FMT_S16P: i32 = 6;
pub const AV_SAMPLE_FMT_S32P: i32 = 7;
pub const AV_SAMPLE_FMT_FLTP: i32 = 8;
pub const AV_SAMPLE_FMT_DBLP: i32 = 9;
pub const AV_SAMPLE_FMT_S64: i32 = 10;
pub const AV_SAMPLE_FMT_S64P: i32 = 11;

pub const AV_PROFILE_AAC_HE: c_int = 4;
pub const AV_PROFILE_AAC_HE_V2: c_int = 28;

pub type AVDiscard = c_int;
pub const AVDISCARD_DEFAULT: i32 = 0;
pub const AVDISCARD_ALL: i32 = 48;

pub const AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX: i32 = 0x01;
pub const AV_CODEC_HW_CONFIG_METHOD_HW_FRAMES_CTX: i32 = 0x02;
pub const AV_CODEC_HW_CONFIG_METHOD_INTERNAL: i32 = 0x04;
pub const AV_CODEC_HW_CONFIG_METHOD_AD_HOC: i32 = 0x08;

pub const AV_CODEC_FLAG_OUTPUT_CORRUPT: i32 = 1 << 3;
pub const AV_CODEC_FLAG_LOW_DELAY: i32 = 1 << 19;
pub const AV_CODEC_FLAG_GLOBAL_HEADER: i32 = 1 << 22;

pub const AV_EF_CRCCHECK: c_int = 1 << 0;
pub const AV_EF_BITSTREAM: c_int = 1 << 1;
pub const AV_EF_BUFFER: c_int = 1 << 2;
pub const AV_EF_EXPLODE: c_int = 1 << 3;

pub const AV_CODEC_CAP_SMALL_LAST_FRAME: i32 = 1 << 6;
pub const AV_CODEC_CAP_VARIABLE_FRAME_SIZE: i32 = 1 << 16;
pub const AV_CODEC_CAP_HARDWARE: i32 = 1 << 18;
pub const AV_CODEC_CAP_ENCODER_FLUSH: i32 = 1 << 21;

pub type AVCodecConfig = c_int;
pub const AV_CODEC_CONFIG_PIX_FORMAT: AVCodecConfig = 0;
pub const AV_CODEC_CONFIG_SAMPLE_RATE: AVCodecConfig = 2;
pub const AV_CODEC_CONFIG_SAMPLE_FORMAT: AVCodecConfig = 3;
pub const AV_CODEC_CONFIG_CHANNEL_LAYOUT: AVCodecConfig = 4;

pub const FF_QP2LAMBDA: c_int = 118;
pub const AV_CODEC_FLAG_QSCALE: c_int = 1 << 1;

pub const AV_PKT_FLAG_KEY: i32 = 0x0001;

pub const AV_INPUT_BUFFER_PADDING_SIZE: i32 = 64;

pub const FF_THREAD_FRAME: i32 = 1;
pub const FF_THREAD_SLICE: i32 = 2;

#[repr(C)]
pub struct RcOverride {
    pub start_frame: c_int,
    pub end_frame: c_int,
    pub qscale: c_int,
    pub quality_factor: f32,
}

#[repr(C)]
pub struct AVCodec {
    pub name: *const c_char,
    pub long_name: *const c_char,
    pub r#type: AVMediaType,
    pub id: AVCodecID,
    pub capabilities: c_int,
}

#[repr(C)]
pub struct AVCodecHWConfig {
    pub pix_fmt: AVPixelFormat,
    pub methods: c_int,
    pub device_type: AVHWDeviceType,
}

#[repr(C)]
pub struct AVCodecContext {
    pub av_class: *const AVClass,
    pub log_level_offset: c_int,

    pub codec_type: AVMediaType,
    pub codec: *const AVCodec,
    pub codec_id: AVCodecID,

    pub codec_tag: c_uint,

    pub priv_data: *mut c_void,

    pub internal: *mut AVCodecInternal,

    pub opaque: *mut c_void,

    pub bit_rate: i64,

    pub flags: c_int,

    pub flags2: c_int,

    pub extradata: *mut u8,
    pub extradata_size: c_int,

    pub time_base: AVRational,

    pub pkt_timebase: AVRational,

    pub framerate: AVRational,

    pub delay: c_int,

    pub width: c_int,
    pub height: c_int,

    pub coded_width: c_int,
    pub coded_height: c_int,

    pub sample_aspect_ratio: AVRational,

    pub pix_fmt: AVPixelFormat,

    pub sw_pix_fmt: AVPixelFormat,

    pub color_primaries: AVColorPrimaries,

    pub color_trc: AVColorTransferCharacteristic,

    pub colorspace: AVColorSpace,

    pub color_range: AVColorRange,

    pub chroma_sample_location: AVChromaLocation,

    pub field_order: AVFieldOrder,

    pub refs: c_int,

    pub has_b_frames: c_int,

    pub slice_flags: c_int,

    pub draw_horiz_band: Option<
        unsafe extern "C" fn(
            s: *mut AVCodecContext,
            src: *const AVFrame,
            offset: *mut c_int,
            y: c_int,
            r#type: c_int,
            height: c_int,
        ),
    >,

    pub get_format: Option<
        unsafe extern "C" fn(s: *mut AVCodecContext, fmt: *const AVPixelFormat) -> AVPixelFormat,
    >,

    pub max_b_frames: c_int,

    pub b_quant_factor: f32,

    pub b_quant_offset: f32,

    pub i_quant_factor: f32,

    pub i_quant_offset: f32,

    pub lumi_masking: f32,

    pub temporal_cplx_masking: f32,

    pub spatial_cplx_masking: f32,

    pub p_masking: f32,

    pub dark_masking: f32,

    pub nsse_weight: c_int,

    pub me_cmp: c_int,
    pub me_sub_cmp: c_int,
    pub mb_cmp: c_int,
    pub ildct_cmp: c_int,

    pub dia_size: c_int,

    pub last_predictor_count: c_int,

    pub me_pre_cmp: c_int,

    pub pre_dia_size: c_int,

    pub me_subpel_quality: c_int,

    pub me_range: c_int,

    pub mb_decision: c_int,

    pub intra_matrix: *mut u16,

    pub inter_matrix: *mut u16,

    pub chroma_intra_matrix: *mut u16,

    pub intra_dc_precision: c_int,

    pub mb_lmin: c_int,

    pub mb_lmax: c_int,

    pub bidir_refine: c_int,

    pub keyint_min: c_int,

    pub gop_size: c_int,

    pub mv0_threshold: c_int,

    pub slices: c_int,

    pub sample_rate: c_int,

    pub sample_fmt: AVSampleFormat,

    pub ch_layout: AVChannelLayout,

    pub frame_size: c_int,

    pub block_align: c_int,

    pub cutoff: c_int,

    pub audio_service_type: AVAudioServiceType,

    pub request_sample_fmt: AVSampleFormat,

    pub initial_padding: c_int,

    pub trailing_padding: c_int,

    pub seek_preroll: c_int,

    pub get_buffer2: Option<
        unsafe extern "C" fn(s: *mut AVCodecContext, frame: *mut AVFrame, flags: c_int) -> c_int,
    >,

    pub bit_rate_tolerance: c_int,

    pub global_quality: c_int,

    pub compression_level: c_int,

    pub qcompress: f32,
    pub qblur: f32,

    pub qmin: c_int,

    pub qmax: c_int,

    pub max_qdiff: c_int,

    pub rc_buffer_size: c_int,

    pub rc_override_count: c_int,
    pub rc_override: *mut RcOverride,

    pub rc_max_rate: i64,

    pub rc_min_rate: i64,

    pub rc_max_available_vbv_use: f32,

    pub rc_min_vbv_overflow_use: f32,

    pub rc_initial_buffer_occupancy: c_int,

    pub trellis: c_int,

    pub stats_out: *mut c_char,

    pub stats_in: *mut c_char,

    pub workaround_bugs: c_int,

    pub strict_std_compliance: c_int,

    pub error_concealment: c_int,

    pub debug: c_int,

    pub err_recognition: c_int,

    pub hwaccel: *const AVHWAccel,

    pub hwaccel_context: *mut c_void,

    pub hw_frames_ctx: *mut AVBufferRef,

    pub hw_device_ctx: *mut AVBufferRef,

    pub hwaccel_flags: c_int,

    pub extra_hw_frames: c_int,

    pub error: [u64; AV_NUM_DATA_POINTERS],

    pub dct_algo: c_int,

    pub idct_algo: c_int,

    pub bits_per_coded_sample: c_int,

    pub bits_per_raw_sample: c_int,

    pub thread_count: c_int,

    pub thread_type: c_int,

    pub active_thread_type: c_int,

    pub execute: Option<
        unsafe extern "C" fn(
            c: *mut AVCodecContext,
            func: Option<unsafe extern "C" fn(c2: *mut AVCodecContext, arg: *mut c_void) -> c_int>,
            arg2: *mut c_void,
            ret: *mut c_int,
            count: c_int,
            size: c_int,
        ) -> c_int,
    >,

    pub execute2: Option<
        unsafe extern "C" fn(
            c: *mut AVCodecContext,
            func: Option<
                unsafe extern "C" fn(
                    c2: *mut AVCodecContext,
                    arg: *mut c_void,
                    jobnr: c_int,
                    threadnr: c_int,
                ) -> c_int,
            >,
            arg2: *mut c_void,
            ret: *mut c_int,
            count: c_int,
        ) -> c_int,
    >,

    pub profile: c_int,

    pub level: c_int,

    pub skip_loop_filter: AVDiscard,

    pub skip_idct: AVDiscard,

    pub skip_frame: AVDiscard,

    pub skip_alpha: c_int,

    pub skip_top: c_int,

    pub skip_bottom: c_int,

    pub lowres: c_int,

    pub codec_descriptor: *const AVCodecDescriptor,

    pub sub_charenc: *mut c_char,

    pub sub_charenc_mode: c_int,

    pub subtitle_header_size: c_int,
    pub subtitle_header: *mut u8,

    pub dump_separator: *mut u8,

    pub codec_whitelist: *mut c_char,

    pub coded_side_data: *mut AVPacketSideData,
    pub nb_coded_side_data: c_int,

    pub export_side_data: c_int,

    pub max_pixels: i64,

    pub apply_cropping: c_int,

    pub discard_damaged_percentage: c_int,

    pub max_samples: i64,

    pub get_encode_buffer: Option<
        unsafe extern "C" fn(s: *mut AVCodecContext, pkt: *mut AVPacket, flags: c_int) -> c_int,
    >,

    pub frame_num: i64,

    pub side_data_prefer_packet: *mut c_int,
    pub nb_side_data_prefer_packet: c_uint,

    pub decoded_side_data: *mut *mut AVFrameSideData,
    pub nb_decoded_side_data: c_int,

    pub alpha_mode: AVAlphaMode,
}

#[repr(C)]
pub struct AVCodecParameters {
    pub codec_type: AVMediaType,
    pub codec_id: AVCodecID,
    pub codec_tag: u32,

    pub extradata: *mut u8,
    pub extradata_size: c_int,

    pub coded_side_data: *mut AVPacketSideData,

    pub nb_coded_side_data: c_int,

    pub format: c_int,

    pub bit_rate: i64,

    pub bits_per_coded_sample: c_int,

    pub bits_per_raw_sample: c_int,

    pub profile: c_int,
    pub level: c_int,

    pub width: c_int,
    pub height: c_int,

    pub sample_aspect_ratio: AVRational,

    pub framerate: AVRational,

    pub field_order: AVFieldOrder,

    pub color_range: AVColorRange,
    pub color_primaries: AVColorPrimaries,
    pub color_trc: AVColorTransferCharacteristic,
    pub color_space: AVColorSpace,
    pub chroma_location: AVChromaLocation,

    pub video_delay: c_int,

    pub ch_layout: AVChannelLayout,

    pub sample_rate: c_int,

    pub block_align: c_int,

    pub frame_size: c_int,

    pub initial_padding: c_int,

    pub trailing_padding: c_int,

    pub seek_preroll: c_int,

    pub alpha_mode: AVAlphaMode,
}

#[repr(C)]
pub struct AVPacketSideData {
    pub data: *mut u8,
    pub size: usize,
    pub r#type: AVPacketSideDataType,
}

#[repr(C)]
pub struct AVPacket {
    pub buf: *mut AVBufferRef,

    pub pts: i64,

    pub dts: i64,
    pub data: *mut u8,
    pub size: c_int,
    pub stream_index: c_int,

    pub flags: c_int,

    pub side_data: *mut AVPacketSideData,
    pub side_data_elems: c_int,

    pub duration: i64,

    pub pos: i64,

    pub opaque: *mut c_void,

    pub opaque_ref: *mut AVBufferRef,

    pub time_base: AVRational,
}

pub type AVPacketSideDataType = c_int;
pub const AV_PKT_DATA_DISPLAYMATRIX: AVPacketSideDataType = 5;
pub const AV_PKT_DATA_STEREO3D: AVPacketSideDataType = 6;
pub const AV_PKT_DATA_SKIP_SAMPLES: AVPacketSideDataType = 11;
pub const AV_PKT_DATA_MASTERING_DISPLAY_METADATA: AVPacketSideDataType = 20;
pub const AV_PKT_DATA_SPHERICAL: AVPacketSideDataType = 21;
pub const AV_PKT_DATA_CONTENT_LIGHT_LEVEL: AVPacketSideDataType = 22;

symbols::symbols! {
    symbols::AVCODEC;

    fn avcodec_version() -> c_uint;

    fn avcodec_alloc_context3(codec: *const AVCodec) -> *mut AVCodecContext;
    fn avcodec_free_context(avctx: *mut *mut AVCodecContext);
    fn avcodec_open2(
        avctx: *mut AVCodecContext,
        codec: *const AVCodec,
        options: *mut *mut AVDictionary,
    ) -> c_int;

    fn avcodec_parameters_alloc() -> *mut AVCodecParameters;
    fn avcodec_parameters_free(par: *mut *mut AVCodecParameters);
    fn avcodec_parameters_to_context(
        codec: *mut AVCodecContext,
        par: *const AVCodecParameters,
    ) -> c_int;
    fn avcodec_parameters_from_context(
        par: *mut AVCodecParameters,
        codec: *const AVCodecContext,
    ) -> c_int;
    fn avcodec_parameters_copy(
        dst: *mut AVCodecParameters,
        src: *const AVCodecParameters,
    ) -> c_int;

    fn avcodec_send_packet(avctx: *mut AVCodecContext, avpkt: *const AVPacket) -> c_int;
    fn avcodec_receive_frame(avctx: *mut AVCodecContext, frame: *mut AVFrame) -> c_int;
    fn avcodec_send_frame(avctx: *mut AVCodecContext, frame: *const AVFrame) -> c_int;
    fn avcodec_receive_packet(avctx: *mut AVCodecContext, avpkt: *mut AVPacket)
    -> c_int;
    fn avcodec_flush_buffers(avctx: *mut AVCodecContext);

    fn avcodec_find_decoder(id: AVCodecID) -> *const AVCodec;
    fn avcodec_find_decoder_by_name(name: *const c_char) -> *const AVCodec;
    fn avcodec_find_encoder(id: AVCodecID) -> *const AVCodec;
    fn avcodec_find_encoder_by_name(name: *const c_char) -> *const AVCodec;

    fn avcodec_get_hw_config(
        codec: *const AVCodec,
        index: c_int,
    ) -> *const AVCodecHWConfig;
    fn avcodec_get_name(id: AVCodecID) -> *const c_char;
    fn avcodec_descriptor_get(id: AVCodecID) -> *const AVCodecDescriptor;
    fn avcodec_get_hw_frames_parameters(
        avctx: *mut AVCodecContext,
        device_ref: *mut AVBufferRef,
        hw_pix_fmt: AVPixelFormat,
        out_frames_ref: *mut *mut AVBufferRef,
    ) -> c_int;

    fn av_packet_alloc() -> *mut AVPacket;
    fn av_packet_free(pkt: *mut *mut AVPacket);
    fn av_packet_unref(pkt: *mut AVPacket);
    fn av_packet_rescale_ts(pkt: *mut AVPacket, tb_src: AVRational, tb_dst: AVRational);
    fn av_packet_side_data_new(
        psd: *mut *mut AVPacketSideData,
        pnb_sd: *mut c_int,
        side_data_type: AVPacketSideDataType,
        size: usize,
        flags: c_int,
    ) -> *mut AVPacketSideData;

    fn avcodec_get_supported_config(
        avctx: *const AVCodecContext,
        codec: *const AVCodec,
        config: AVCodecConfig,
        flags: c_uint,
        out_configs: *mut *const c_void,
        out_num_configs: *mut c_int,
    ) -> c_int;
}
