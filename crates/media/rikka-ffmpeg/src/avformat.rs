#![allow(
    missing_debug_implementations,
    non_camel_case_types,
    non_snake_case,
    dead_code,
    clippy::upper_case_acronyms
)]

use core::ffi::{c_char, c_int, c_uint, c_void};

use crate::avcodec::*;
use crate::avutil::*;
use crate::symbols;

#[repr(C)]
pub struct AVIOContext {
    pub av_class: *const AVClass,
    pub buffer: *mut u8,
    pub buffer_size: c_int,
    pub buf_ptr: *mut u8,
    pub buf_end: *mut u8,
    pub opaque: *mut c_void,
}

#[repr(C)]
pub struct AVProgram {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct AVChapter {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct AVStreamGroup {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct AVCodecTag {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct AVIOInterruptCB {
    pub callback: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
    pub opaque: *mut c_void,
}

#[repr(C)]
pub struct AVIndexEntry {
    pub pos: i64,
    pub timestamp: i64,
}

#[repr(C)]
pub struct AVOutputFormat {
    pub name: *const c_char,
    pub long_name: *const c_char,
    pub mime_type: *const c_char,
    pub extensions: *const c_char,
    pub audio_codec: AVCodecID,
    pub video_codec: AVCodecID,
    pub subtitle_codec: AVCodecID,
    pub flags: c_int,
}

#[repr(C)]
pub struct AVInputFormat {
    pub name: *const c_char,
    pub long_name: *const c_char,
    pub flags: c_int,
}

#[repr(C)]
pub struct AVStream {
    pub av_class: *const AVClass,
    pub index: c_int,
    pub id: c_int,
    pub codecpar: *mut AVCodecParameters,
    pub priv_data: *mut c_void,
    pub time_base: AVRational,
    pub start_time: i64,
    pub duration: i64,
    pub nb_frames: i64,
    pub disposition: c_int,
    pub discard: AVDiscard,
    pub sample_aspect_ratio: AVRational,
    pub metadata: *mut AVDictionary,
    pub avg_frame_rate: AVRational,
    pub attached_pic: AVPacket,
    pub event_flags: c_int,
    pub r_frame_rate: AVRational,
    pub pts_wrap_bits: c_int,
}

#[repr(C)]
pub struct AVFormatContext {
    pub av_class: *const AVClass,
    pub iformat: *const AVInputFormat,
    pub oformat: *const AVOutputFormat,
    pub priv_data: *mut c_void,
    pub pb: *mut AVIOContext,
    pub ctx_flags: c_int,
    pub nb_streams: c_uint,
    pub streams: *mut *mut AVStream,
    pub nb_stream_groups: c_uint,
    pub stream_groups: *mut *mut AVStreamGroup,
    pub nb_chapters: c_uint,
    pub chapters: *mut *mut AVChapter,
    pub url: *mut c_char,
    pub start_time: i64,
    pub duration: i64,
    pub bit_rate: i64,
    pub packet_size: c_uint,
    pub max_delay: c_int,
    pub flags: c_int,
    pub probesize: i64,
    pub max_analyze_duration: i64,
    pub key: *const u8,
    pub keylen: c_int,
    pub nb_programs: c_uint,
    pub programs: *mut *mut AVProgram,
    pub video_codec_id: AVCodecID,
    pub audio_codec_id: AVCodecID,
    pub subtitle_codec_id: AVCodecID,
    pub data_codec_id: AVCodecID,
    pub metadata: *mut AVDictionary,
    pub start_time_realtime: i64,
    pub fps_probe_size: c_int,
    pub error_recognition: c_int,
    pub interrupt_callback: AVIOInterruptCB,
    pub debug: c_int,
    pub max_streams: c_int,
    pub max_index_size: c_uint,
    pub max_picture_buffer: c_uint,
    pub max_interleave_delta: i64,
    pub max_ts_probe: c_int,
    pub max_chunk_duration: c_int,
    pub max_chunk_size: c_int,
    pub max_probe_packets: c_int,
    pub strict_std_compliance: c_int,
    pub event_flags: c_int,
    pub avoid_negative_ts: c_int,
    pub audio_preload: c_int,
    pub use_wallclock_as_timestamps: c_int,
    pub skip_estimate_duration_from_pts: c_int,
    pub avio_flags: c_int,
    pub duration_estimation_method: c_int,
    pub skip_initial_bytes: i64,
    pub correct_ts_overflow: c_uint,
    pub seek2any: c_int,
    pub flush_packets: c_int,
    pub probe_score: c_int,
    pub format_probesize: c_int,
    pub codec_whitelist: *mut c_char,
    pub format_whitelist: *mut c_char,
    pub protocol_whitelist: *mut c_char,
    pub protocol_blacklist: *mut c_char,
    pub io_repositioned: c_int,
    pub video_codec: *const AVCodec,
    pub audio_codec: *const AVCodec,
    pub subtitle_codec: *const AVCodec,
    pub data_codec: *const AVCodec,
    pub metadata_header_padding: c_int,
    pub opaque: *mut c_void,
    pub control_message_cb:
        Option<unsafe extern "C" fn(*mut AVFormatContext, c_int, *mut c_void, usize) -> c_int>,
    pub output_ts_offset: i64,
    pub dump_separator: *mut u8,
    pub io_open: Option<
        unsafe extern "C" fn(
            *mut AVFormatContext,
            *mut *mut AVIOContext,
            *const c_char,
            c_int,
            *mut *mut AVDictionary,
        ) -> c_int,
    >,
    pub io_close2: Option<unsafe extern "C" fn(*mut AVFormatContext, *mut AVIOContext) -> c_int>,
    pub duration_probesize: i64,
    pub name: *mut c_char,
}

pub const AVFMT_NOFILE: c_int = 0x0001;
pub const AVFMT_GLOBALHEADER: c_int = 0x0040;
pub const AVFMT_VARIABLE_FPS: c_int = 0x0400;

pub const AVSEEK_FLAG_BACKWARD: c_int = 1;
pub const AVSEEK_FLAG_BYTE: c_int = 2;
pub const AVSEEK_FLAG_ANY: c_int = 4;
pub const AVSEEK_FLAG_FRAME: c_int = 8;

pub const AVIO_FLAG_READ: c_int = 1;
pub const AVIO_FLAG_WRITE: c_int = 2;
pub const AVIO_FLAG_READ_WRITE: c_int = 3;

pub const AV_DISPOSITION_DEFAULT: c_int = 1 << 0;
pub const AV_DISPOSITION_ATTACHED_PIC: c_int = 1 << 10;

pub const AVFMT_FLAG_GENPTS: c_int = 0x0001;
pub const AVFMT_FLAG_CUSTOM_IO: c_int = 0x0080;

pub const AVSEEK_SIZE: c_int = 0x10000;
pub const AVSEEK_FORCE: c_int = 0x20000;

symbols::symbols! {
    symbols::AVFORMAT;

    fn avformat_version() -> c_uint;

    fn avformat_open_input(
        ps: *mut *mut AVFormatContext,
        url: *const c_char,
        fmt: *const AVInputFormat,
        options: *mut *mut AVDictionary,
    ) -> c_int;
    fn avformat_close_input(s: *mut *mut AVFormatContext);
    fn avformat_find_stream_info(
        ic: *mut AVFormatContext,
        options: *mut *mut AVDictionary,
    ) -> c_int;
    fn av_find_best_stream(
        ic: *mut AVFormatContext,
        media_type: AVMediaType,
        wanted_stream_nb: c_int,
        related_stream: c_int,
        decoder_ret: *mut *const AVCodec,
        flags: c_int,
    ) -> c_int;
    fn av_read_frame(s: *mut AVFormatContext, pkt: *mut AVPacket) -> c_int;

    fn av_seek_frame(
        s: *mut AVFormatContext,
        stream_index: c_int,
        timestamp: i64,
        flags: c_int,
    ) -> c_int;
    fn avformat_seek_file(
        s: *mut AVFormatContext,
        stream_index: c_int,
        min_ts: i64,
        ts: i64,
        max_ts: i64,
        flags: c_int,
    ) -> c_int;
    fn avformat_flush(s: *mut AVFormatContext) -> c_int;
    fn avformat_index_get_entry_from_timestamp(
        st: *mut AVStream,
        wanted_timestamp: i64,
        flags: c_int,
    ) -> *const AVIndexEntry;

    fn avformat_alloc_output_context2(
        ctx: *mut *mut AVFormatContext,
        oformat: *const AVOutputFormat,
        format_name: *const c_char,
        filename: *const c_char,
    ) -> c_int;
    fn avformat_alloc_context() -> *mut AVFormatContext;
    fn avformat_free_context(s: *mut AVFormatContext);
    fn avformat_new_stream(s: *mut AVFormatContext, c: *const AVCodec) -> *mut AVStream;

    fn avformat_write_header(
        s: *mut AVFormatContext,
        options: *mut *mut AVDictionary,
    ) -> c_int;
    fn av_interleaved_write_frame(s: *mut AVFormatContext, pkt: *mut AVPacket) -> c_int;
    fn av_write_trailer(s: *mut AVFormatContext) -> c_int;
    fn avformat_query_codec(
        ofmt: *const AVOutputFormat,
        codec_id: AVCodecID,
        std_compliance: c_int,
    ) -> c_int;
    fn avio_open(s: *mut *mut AVIOContext, url: *const c_char, flags: c_int) -> c_int;
    fn avio_closep(s: *mut *mut AVIOContext) -> c_int;
    fn avio_alloc_context(
        buffer: *mut u8,
        buffer_size: c_int,
        write_flag: c_int,
        opaque: *mut c_void,
        read_packet: Option<unsafe extern "C" fn(*mut c_void, *mut u8, c_int) -> c_int>,
        write_packet: Option<unsafe extern "C" fn(*mut c_void, *const u8, c_int) -> c_int>,
        seek: Option<unsafe extern "C" fn(*mut c_void, i64, c_int) -> i64>,
    ) -> *mut AVIOContext;
    fn avio_context_free(s: *mut *mut AVIOContext);

    fn av_dump_format(
        ic: *mut AVFormatContext,
        index: c_int,
        url: *const c_char,
        is_output: c_int,
    );
}
