use std::path::Path;
use std::ptr;

use crate::error::{Result, av_err, check};
use rikka_ffmpeg::InputContext;
use rikka_ffmpeg::Packet;
use rikka_ffmpeg::avcodec::{self, AVCodec};
use rikka_ffmpeg::avformat::{self, AVStream};
use rikka_ffmpeg::avutil::{self, AVRational};

const NOMINAL: std::ops::RangeInclusive<f64> = 1.0..=240.0;

fn per_second(rate: AVRational) -> f64 {
    match rate.num > 0 && rate.den > 0 {
        true => f64::from(rate.num) / f64::from(rate.den),
        false => 0.0,
    }
}

pub struct Demuxer {
    ctx: InputContext,
    stream_index: i32,
    decoder: *const AVCodec,
}

unsafe impl Send for Demuxer {}

impl Demuxer {
    #[must_use]
    pub fn supports_vulkan(&self) -> bool {
        unsafe { crate::decode::supports_vulkan(self.decoder) }
    }

    pub fn open(path: &Path) -> Result<Self> {
        let ctx = InputContext::open(path)?;
        let result = unsafe { avformat::avformat_find_stream_info(ctx.as_ptr(), ptr::null_mut()) };
        check("avformat_find_stream_info", result)?;

        let mut decoder: *const AVCodec = ptr::null();
        let stream_index = unsafe {
            avformat::av_find_best_stream(
                ctx.as_ptr(),
                avutil::AVMEDIA_TYPE_VIDEO,
                -1,
                -1,
                &mut decoder,
                0,
            )
        };
        if stream_index < 0 || decoder.is_null() {
            return Err(av_err("av_find_best_stream", stream_index));
        }

        Ok(Self {
            ctx,
            stream_index,
            decoder,
        })
    }

    pub fn decoder(&self) -> *const AVCodec {
        self.decoder
    }

    pub fn stream(&self) -> &AVStream {
        self.stream_at(self.stream_index as usize)
    }

    pub fn video_index(&self) -> i32 {
        self.stream_index
    }

    pub fn stream_count(&self) -> usize {
        unsafe { (*self.ctx.as_ptr()).nb_streams as usize }
    }

    pub fn stream_at(&self, index: usize) -> &AVStream {
        unsafe { &**(*self.ctx.as_ptr()).streams.add(index) }
    }

    pub fn read_only_video(&mut self) {
        for index in 0..self.stream_count() {
            if index as i32 != self.stream_index {
                unsafe {
                    (**(*self.ctx.as_ptr()).streams.add(index)).discard = avcodec::AVDISCARD_ALL;
                }
            }
        }
    }

    pub fn metadata(&self) -> *const avutil::AVDictionary {
        unsafe { (*self.ctx.as_ptr()).metadata }
    }

    pub fn time_base(&self) -> AVRational {
        self.stream().time_base
    }

    pub fn start_us(&self) -> Option<i64> {
        let start = unsafe { (*self.ctx.as_ptr()).start_time };
        (start != avutil::AV_NOPTS_VALUE).then_some(start)
    }

    pub fn keyframe(&self, timestamp: i64, at_or_before: bool) -> Option<i64> {
        let flags = match at_or_before {
            true => avformat::AVSEEK_FLAG_BACKWARD,
            false => 0,
        };
        let stream = unsafe { *(*self.ctx.as_ptr()).streams.add(self.stream_index as usize) };
        let entry =
            unsafe { avformat::avformat_index_get_entry_from_timestamp(stream, timestamp, flags) };
        (!entry.is_null()).then(|| unsafe { (*entry).timestamp })
    }

    pub fn duration_us(&self) -> Option<i64> {
        let duration = unsafe { (*self.ctx.as_ptr()).duration };
        (duration != avutil::AV_NOPTS_VALUE && duration > 0).then_some(duration)
    }

    pub fn frame_rate(&self) -> f64 {
        let stream = self.stream();
        let nominal = per_second(stream.r_frame_rate);
        match NOMINAL.contains(&nominal) {
            true => nominal,
            false => per_second(stream.avg_frame_rate),
        }
    }

    pub fn read(&mut self, packet: &mut Packet) -> Result<bool> {
        while self.read_any(packet)? {
            if packet.stream_index() == self.stream_index {
                return Ok(true);
            }
            packet.unref();
        }
        Ok(false)
    }

    pub fn read_any(&mut self, packet: &mut Packet) -> Result<bool> {
        let result = unsafe { avformat::av_read_frame(self.ctx.as_ptr(), packet.as_mut_ptr()) };
        if result == avutil::AVERROR_EOF {
            return Ok(false);
        }
        check("av_read_frame", result)?;
        Ok(true)
    }

    pub fn seek_us(&mut self, target_us: i64) -> Result<()> {
        self.seek_keyframe(target_us, avformat::AVSEEK_FLAG_BACKWARD)
    }

    pub fn seek_ahead_us(&mut self, target_us: i64) -> Result<()> {
        self.seek_keyframe(target_us, 0)
    }

    fn seek_keyframe(&mut self, target_us: i64, flags: core::ffi::c_int) -> Result<()> {
        let ts =
            unsafe { avutil::av_rescale_q(target_us, avutil::AV_TIME_BASE_Q, self.time_base()) };
        let result =
            unsafe { avformat::av_seek_frame(self.ctx.as_ptr(), self.stream_index, ts, flags) };
        check("av_seek_frame", result)?;
        Ok(())
    }
}

impl std::fmt::Debug for Demuxer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Demuxer").finish_non_exhaustive()
    }
}
