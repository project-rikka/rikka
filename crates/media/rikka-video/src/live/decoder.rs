use std::ffi::CString;
use std::ptr;

use crate::encode::set_option;
use crate::error::{Error, Result, check};
use crate::live::exchange;
use rikka_ffmpeg::CodecContext;
use rikka_ffmpeg::Frame;
use rikka_ffmpeg::Packet;
use rikka_ffmpeg::avcodec::{self, AVCodec, AVCodecID};
use rikka_video_core::init;
use rikka_video_core::sw::SwConverter;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LiveCodec {
    H264,
    Hevc,
    Vp8,
    Vp9,
    Av1,
}

impl LiveCodec {
    pub(crate) fn id(self) -> AVCodecID {
        match self {
            LiveCodec::H264 => avcodec::AV_CODEC_ID_H264,
            LiveCodec::Hevc => avcodec::AV_CODEC_ID_HEVC,
            LiveCodec::Vp8 => avcodec::AV_CODEC_ID_VP8,
            LiveCodec::Vp9 => avcodec::AV_CODEC_ID_VP9,
            LiveCodec::Av1 => avcodec::AV_CODEC_ID_AV1,
        }
    }

    fn preferred(self) -> &'static [&'static str] {
        match self {
            LiveCodec::Av1 => &["libdav1d", "libaom-av1"],
            LiveCodec::H264 | LiveCodec::Hevc | LiveCodec::Vp8 | LiveCodec::Vp9 => &[],
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct LivePicture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for LivePicture {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LivePicture")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

pub struct LiveDecoder {
    codec: CodecContext,
    name: String,
    packet: Packet,
    frame: Frame,
    converter: SwConverter,
}

impl LiveDecoder {
    pub fn open(codec: LiveCodec) -> Result<LiveDecoder> {
        init::ensure()?;
        let mut last_error = None;
        for found in candidates(codec) {
            match open_one(found) {
                Ok(decoder) => return Ok(decoder),
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error
            .unwrap_or_else(|| Error::Unsupported(format!("this FFmpeg cannot decode {codec:?}"))))
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn decode(&mut self, data: &[u8], within: Option<(u32, u32)>) -> Result<Vec<LivePicture>> {
        let mut pictures = Vec::new();
        let converter = &mut self.converter;
        exchange::decode_packet(
            &self.codec,
            &mut self.packet,
            data,
            &mut self.frame,
            |frame| {
                let mut rgba = Vec::new();
                let shown = converter.rgba_within(frame, within, &mut rgba);
                frame.unref();
                let (width, height) = shown?;
                pictures.push(LivePicture {
                    width,
                    height,
                    rgba,
                });
                Ok(())
            },
        )?;
        Ok(pictures)
    }
}

impl std::fmt::Debug for LiveDecoder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LiveDecoder")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

fn candidates(codec: LiveCodec) -> Vec<*const AVCodec> {
    let mut found: Vec<*const AVCodec> = codec
        .preferred()
        .iter()
        .filter_map(|name| CString::new(*name).ok())
        .map(|name| unsafe { avcodec::avcodec_find_decoder_by_name(name.as_ptr()) })
        .filter(|decoder| !decoder.is_null())
        .collect();
    let native = unsafe { avcodec::avcodec_find_decoder(codec.id()) };
    if !native.is_null() && !found.contains(&native) {
        found.push(native);
    }
    found
}

fn open_one(codec: *const AVCodec) -> Result<LiveDecoder> {
    let context = unsafe { CodecContext::alloc(codec) }?;
    let raw = context.as_ptr();
    let name = unsafe {
        std::ffi::CStr::from_ptr((*codec).name)
            .to_string_lossy()
            .into_owned()
    };
    unsafe {
        exchange::refuse_damage(raw);
        (*raw).thread_count = 0;
        (*raw).thread_type = avcodec::FF_THREAD_SLICE;
        if !(*raw).priv_data.is_null() && name == "libdav1d" {
            set_option((*raw).priv_data, "max_frame_delay", "1");
        }
    }
    let status = unsafe { avcodec::avcodec_open2(raw, codec, ptr::null_mut()) };
    check("avcodec_open2", status)?;
    Ok(LiveDecoder {
        codec: context,
        name,
        packet: Packet::new()?,
        frame: Frame::new()?,
        converter: SwConverter::new(),
    })
}
