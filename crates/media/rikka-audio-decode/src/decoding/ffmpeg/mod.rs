mod codec;
mod samples;

use core::ffi::{CStr, c_int};
use std::ffi::CString;
use std::ptr;
use std::time::Duration;

use rikka_ffmpeg::avcodec::{self, AVCodec, AVCodecParameters, AVDISCARD_ALL};
use rikka_ffmpeg::avformat::{self, AVStream};
use rikka_ffmpeg::avutil::{self, AV_NOPTS_VALUE, AVMEDIA_TYPE_AUDIO};
use rikka_ffmpeg::{CodecContext, Frame, InputContext, Packet};

use crate::decoding::backend::{Backend, Block, Opened};
use crate::decoding::clock::Clock;
use crate::decoding::source::Shared;
use crate::encoding::{Codec, Encoding};
use crate::error::{Error, Result};
use crate::read::AudioInfo;

const PREROLL: u64 = 4096;
const AVFMT_DURATION_FROM_STREAM: c_int = 1;
const GAPLESS: &CStr = c"iTunSMPB";
const GAPLESS_LENGTH: usize = 3;

struct Demuxed {
    codec: CodecContext,
    input: InputContext,
    stream: c_int,
    seekable: bool,
}

impl Demuxed {
    fn open(source: Shared, name: Option<&CStr>, sniffed: Option<Encoding>) -> Result<Demuxed> {
        let seekable = source.len().is_some();
        let input = InputContext::open_source(Box::new(source), name)?;
        let ctx = input.as_ptr();
        rikka_ffmpeg::check("avformat_find_stream_info", unsafe {
            avformat::avformat_find_stream_info(ctx, ptr::null_mut())
        })?;
        let mut decoder: *const AVCodec = ptr::null();
        let found = unsafe {
            avformat::av_find_best_stream(ctx, AVMEDIA_TYPE_AUDIO, -1, -1, &mut decoder, 0)
        };
        if found == avutil::AVERROR_DECODER_NOT_FOUND {
            let stream = first_audio(&input).ok_or(Error::NoAudioTrack)?;
            let id = unsafe { (*(*stream).codecpar).codec_id };
            let encoding = codec::encoding(id, 0, sniffed);
            return Err(Error::NoDecoder(encoding.codec));
        }
        if found < 0 || decoder.is_null() {
            return Err(Error::NoAudioTrack);
        }
        let streams =
            unsafe { std::slice::from_raw_parts((*ctx).streams, (*ctx).nb_streams as usize) };
        for (index, stream) in streams.iter().enumerate() {
            if index as c_int != found {
                unsafe { (**stream).discard = AVDISCARD_ALL };
            }
        }
        let stream = streams[found as usize];
        let codec = unsafe { CodecContext::alloc(decoder) }?;
        unsafe {
            rikka_ffmpeg::check(
                "avcodec_parameters_to_context",
                avcodec::avcodec_parameters_to_context(codec.as_ptr(), (*stream).codecpar),
            )?;
            (*codec.as_ptr()).pkt_timebase = (*stream).time_base;
            rikka_ffmpeg::check(
                "avcodec_open2",
                avcodec::avcodec_open2(codec.as_ptr(), decoder, ptr::null_mut()),
            )?;
        }
        Ok(Demuxed {
            codec,
            input,
            stream: found,
            seekable,
        })
    }

    fn stream(&self) -> *mut AVStream {
        let ctx = self.input.as_ptr();
        unsafe { *(*ctx).streams.add(self.stream as usize) }
    }
}

struct FfmpegBackend {
    source: Shared,
    name: Option<CString>,
    sniffed: Option<Encoding>,
    demuxed: Demuxed,
    packet: Packet,
    frame: Frame,
    clock: Clock,
    origin: i64,
    channels: usize,
    preroll: u64,
    continuing: Option<i64>,
    pending: Option<Block>,
    draining: bool,
    trims_end: bool,
}

pub(crate) fn open(
    source: Shared,
    name: Option<CString>,
    sniffed: Option<Encoding>,
) -> Result<Opened> {
    let length = source.len();
    let demuxed = Demuxed::open(source.clone(), name.as_deref(), sniffed)?;
    let stream = demuxed.stream();
    let parameters = unsafe { (*stream).codecpar };
    let time_base = unsafe { (*stream).time_base };
    let mut backend = FfmpegBackend {
        source,
        name,
        sniffed,
        demuxed,
        packet: Packet::new()?,
        frame: Frame::new()?,
        clock: Clock::new(1, 1, 1),
        origin: 0,
        channels: 0,
        preroll: 0,
        continuing: None,
        pending: None,
        draining: false,
        trims_end: false,
    };
    let mut first = Block::default();
    if !backend.receive()? {
        return Err(Error::NoAudioTrack);
    }
    let rate = u32::try_from(backend.frame.sample_rate()).unwrap_or(0);
    backend.channels = backend.frame.channels();
    if rate == 0 || backend.channels == 0 {
        return Err(Error::NoAudioTrack);
    }
    backend.clock = Clock::new(i64::from(time_base.num), i64::from(time_base.den), rate);
    backend.place(&mut first)?;
    backend.origin = first.start;
    first.start = 0;
    backend.pending = Some(first);

    let profile = unsafe { (*backend.demuxed.codec.as_ptr()).profile };
    let mut encoding = codec::encoding(unsafe { (*parameters).codec_id }, profile, sniffed);
    backend.preroll = match encoding.codec.is_lossless() {
        true => 0,
        false => PREROLL,
    };
    let info = describe(&backend, stream, parameters, rate, length, &mut encoding);
    backend.trims_end = info.frames.is_some();
    Ok(Opened {
        backend: Box::new(backend),
        info,
    })
}

fn describe(
    backend: &FfmpegBackend,
    stream: *mut AVStream,
    parameters: *const AVCodecParameters,
    rate: u32,
    length: Option<u64>,
    encoding: &mut Encoding,
) -> AudioInfo {
    let (span, bits_raw, bits_coded, stream_rate, stream_bitrate) = unsafe {
        (
            (*stream).duration,
            (*parameters).bits_per_raw_sample,
            (*parameters).bits_per_coded_sample,
            (*parameters).sample_rate,
            (*parameters).bit_rate,
        )
    };
    let stated = unsafe { (*backend.demuxed.input.as_ptr()).duration_estimation_method }
        == AVFMT_DURATION_FROM_STREAM;
    let primed = unsafe { (*parameters).initial_padding };
    let frames = gapless(backend)
        .filter(|_| backend.clock.exact())
        .or_else(|| {
            if !stated || span == AV_NOPTS_VALUE || !backend.clock.whole() {
                return None;
            }
            let frames = backend.clock.frames(span);
            let frames = match encoding.codec {
                Codec::Opus => frames - i64::from(primed),
                _ => frames,
            };
            u64::try_from(frames).ok()
        });
    let duration = frames
        .map(|frames| Duration::from_secs_f64(frames as f64 / f64::from(rate)))
        .or_else(|| {
            let base = unsafe { (*stream).time_base };
            (span != AV_NOPTS_VALUE && span > 0 && base.den > 0).then(|| {
                Duration::from_secs_f64(span as f64 * f64::from(base.num) / f64::from(base.den))
            })
        })
        .or_else(|| {
            let whole = unsafe { (*backend.demuxed.input.as_ptr()).duration };
            (whole != AV_NOPTS_VALUE && whole > 0)
                .then(|| Duration::from_secs_f64(whole as f64 / f64::from(avutil::AV_TIME_BASE)))
        });
    let format_bitrate = unsafe { (*backend.demuxed.input.as_ptr()).bit_rate };
    encoding.bitrate = [stream_bitrate, format_bitrate]
        .into_iter()
        .find(|bitrate| *bitrate > 0)
        .and_then(|bitrate| u32::try_from(bitrate).ok())
        .or_else(|| {
            let seconds = duration?.as_secs_f64();
            let length = length?;
            (seconds > 0.0).then(|| (length as f64 * 8.0 / seconds).round() as u32)
        });
    let dsd = encoding.codec == Codec::Dsd;
    if dsd {
        encoding.dsd_rate = u32::try_from(stream_rate).ok().map(|rate| rate * 8);
    }
    let bits = [bits_raw, bits_coded]
        .into_iter()
        .find(|bits| *bits > 0)
        .and_then(|bits| u32::try_from(bits).ok());
    AudioInfo {
        sample_rate: rate,
        channels: backend.channels as u16,
        duration,
        frames,
        bits_per_sample: match (dsd, encoding.lossless) {
            (true, _) => Some(1),
            (false, true) => bits,
            (false, false) => None,
        },
        encoding: *encoding,
    }
}

fn gapless(backend: &FfmpegBackend) -> Option<u64> {
    let metadata = unsafe { (*backend.demuxed.input.as_ptr()).metadata };
    let entry = unsafe { avutil::av_dict_get(metadata, GAPLESS.as_ptr(), ptr::null(), 0) };
    if entry.is_null() {
        return None;
    }
    let text = unsafe { CStr::from_ptr((*entry).value) }.to_str().ok()?;
    let length = text.split_whitespace().nth(GAPLESS_LENGTH)?;
    u64::from_str_radix(length, 16)
        .ok()
        .filter(|frames| *frames > 0)
}

fn first_audio(input: &InputContext) -> Option<*mut AVStream> {
    let ctx = input.as_ptr();
    let streams = unsafe { std::slice::from_raw_parts((*ctx).streams, (*ctx).nb_streams as usize) };
    streams
        .iter()
        .copied()
        .find(|stream| unsafe { (*(**stream).codecpar).codec_type } == AVMEDIA_TYPE_AUDIO)
}

impl FfmpegBackend {
    fn receive(&mut self) -> Result<bool> {
        loop {
            let received = unsafe {
                avcodec::avcodec_receive_frame(self.demuxed.codec.as_ptr(), self.frame.as_mut_ptr())
            };
            match received {
                0 => return Ok(true),
                avutil::AVERROR_EOF => return Ok(false),
                avutil::AVERROR_EAGAIN if self.draining => return Ok(false),
                avutil::AVERROR_EAGAIN => self.feed(),
                error => {
                    tracing::debug!(
                        "FFmpeg failed to decode frame: {}",
                        rikka_ffmpeg::describe(error)
                    );
                    self.continuing = None;
                }
            }
        }
    }

    fn feed(&mut self) {
        loop {
            let read = unsafe {
                avformat::av_read_frame(self.demuxed.input.as_ptr(), self.packet.as_mut_ptr())
            };
            if read < 0 {
                if read != avutil::AVERROR_EOF {
                    tracing::debug!("FFmpeg read stopped: {}", rikka_ffmpeg::describe(read));
                }
                unsafe { avcodec::avcodec_send_packet(self.demuxed.codec.as_ptr(), ptr::null()) };
                self.draining = true;
                return;
            }
            if self.packet.stream_index() != self.demuxed.stream {
                self.packet.unref();
                continue;
            }
            if self.trims_end {
                self.packet.keep_trailing_samples();
            }
            let sent = unsafe {
                avcodec::avcodec_send_packet(self.demuxed.codec.as_ptr(), self.packet.as_mut_ptr())
            };
            self.packet.unref();
            if sent < 0 && sent != avutil::AVERROR_EAGAIN {
                tracing::debug!("FFmpeg rejected packet: {}", rikka_ffmpeg::describe(sent));
                self.continuing = None;
                continue;
            }
            return;
        }
    }

    fn place(&mut self, block: &mut Block) -> Result<()> {
        block.samples.clear();
        if self.frame.channels() != self.channels {
            tracing::warn!("channel count changed mid-stream, skipping frame");
            self.continuing = None;
            return Ok(());
        }
        samples::interleave(&self.frame, &mut block.samples)?;
        let frames = (block.samples.len() / self.channels.max(1)) as i64;
        let stamped = self.frame.timestamp().map(|ticks| self.clock.frames(ticks));
        let from = match (self.clock.exact(), self.continuing, stamped) {
            (true, _, Some(stamped)) | (false, None, Some(stamped)) => stamped,
            (_, Some(next), _) => next,
            (_, None, None) => self.origin,
        };
        self.continuing = Some(from + frames);
        block.start = from - self.origin;
        Ok(())
    }
}

impl FfmpegBackend {
    fn reopen(&mut self) -> Result<()> {
        self.source.rewind()?;
        self.demuxed = Demuxed::open(self.source.clone(), self.name.as_deref(), self.sniffed)?;
        Ok(())
    }
}

impl Backend for FfmpegBackend {
    fn next(&mut self, block: &mut Block) -> Result<bool> {
        if let Some(pending) = self.pending.take() {
            *block = pending;
            return Ok(true);
        }
        loop {
            if !self.receive()? {
                return Ok(false);
            }
            self.place(block)?;
            if !block.samples.is_empty() {
                return Ok(true);
            }
        }
    }

    fn seek(&mut self, frame: u64) -> Result<()> {
        let from = i64::try_from(frame.saturating_sub(self.preroll)).unwrap_or(i64::MAX);
        let ts = self.clock.ticks(from + self.origin);
        if !self.demuxed.seekable && self.source.len().is_some() {
            self.reopen()?;
        }
        let sought = match self.demuxed.seekable {
            true => unsafe {
                avformat::avformat_seek_file(
                    self.demuxed.input.as_ptr(),
                    self.demuxed.stream,
                    i64::MIN,
                    ts,
                    ts,
                    0,
                )
            },
            false => avutil::AVERROR_EIO,
        };
        if sought < 0 {
            tracing::debug!(
                "FFmpeg cannot seek ({}); decoding from the start",
                rikka_ffmpeg::describe(sought)
            );
            self.reopen()?;
        } else {
            unsafe { avcodec::avcodec_flush_buffers(self.demuxed.codec.as_ptr()) };
        }
        self.continuing = None;
        self.pending = None;
        self.draining = false;
        Ok(())
    }
}
