use std::ffi::CStr;
use std::path::Path;
use std::ptr;

use rikka_ffmpeg::avcodec;
use rikka_ffmpeg::avformat::{self, AVStream};
use rikka_ffmpeg::avutil::{self, AVMediaType};
use rikka_ffmpeg::{InputContext, Packet};

use crate::error::{Result, check};

pub(crate) struct Source {
    ctx: InputContext,
    pub(crate) video: Option<usize>,
    pub(crate) audio: Option<usize>,
    pub(crate) start_us: i64,
    pub(crate) duration_us: Option<i64>,
}

impl Source {
    pub(crate) fn open(path: &Path) -> Result<Source> {
        let ctx = InputContext::open(path)?;
        let found = unsafe { avformat::avformat_find_stream_info(ctx.as_ptr(), ptr::null_mut()) };
        check("avformat_find_stream_info", found)?;
        let (start, duration) = unsafe { ((*ctx.as_ptr()).start_time, (*ctx.as_ptr()).duration) };
        let mut source = Source {
            ctx,
            video: None,
            audio: None,
            start_us: match start {
                avutil::AV_NOPTS_VALUE => 0,
                start => start,
            },
            duration_us: (duration != avutil::AV_NOPTS_VALUE && duration > 0).then_some(duration),
        };
        source.video = source
            .best(avutil::AVMEDIA_TYPE_VIDEO, -1)
            .filter(|index| !source.stream(*index).is_picture());
        source.audio = source.best(
            avutil::AVMEDIA_TYPE_AUDIO,
            source.video.map_or(-1, |index| index as i32),
        );
        Ok(source)
    }

    pub(crate) fn streams(&self) -> usize {
        unsafe { (*self.ctx.as_ptr()).nb_streams as usize }
    }

    pub(crate) fn stream(&self, index: usize) -> &AVStream {
        unsafe { &**(*self.ctx.as_ptr()).streams.add(index) }
    }

    pub(crate) fn metadata(&self) -> *const avutil::AVDictionary {
        unsafe { (*self.ctx.as_ptr()).metadata }
    }

    pub(crate) fn read(&mut self, packet: &mut Packet) -> Result<bool> {
        let result = unsafe { avformat::av_read_frame(self.ctx.as_ptr(), packet.as_mut_ptr()) };
        if result == avutil::AVERROR_EOF {
            return Ok(false);
        }
        check("av_read_frame", result)?;
        Ok(true)
    }

    fn best(&self, kind: AVMediaType, related: i32) -> Option<usize> {
        let found = unsafe {
            avformat::av_find_best_stream(self.ctx.as_ptr(), kind, -1, related, ptr::null_mut(), 0)
        };
        usize::try_from(found).ok()
    }
}

pub(crate) trait Described {
    fn kind(&self) -> AVMediaType;
    fn codec_name(&self) -> &'static str;
    fn is_picture(&self) -> bool;
}

impl Described for AVStream {
    fn kind(&self) -> AVMediaType {
        unsafe { (*self.codecpar).codec_type }
    }

    fn codec_name(&self) -> &'static str {
        let name = unsafe { avcodec::avcodec_get_name((*self.codecpar).codec_id) };
        match name.is_null() {
            true => "",
            false => unsafe { CStr::from_ptr(name) }.to_str().unwrap_or(""),
        }
    }

    fn is_picture(&self) -> bool {
        self.disposition & avformat::AV_DISPOSITION_ATTACHED_PIC != 0
    }
}
