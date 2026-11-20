use std::ffi::CStr;
use std::ptr;

use rikka_ffmpeg::avcodec::{self, AVCodec};
use rikka_ffmpeg::avformat::AVStream;
use rikka_ffmpeg::avutil::{self, AVPixelFormat, AVRational};
use rikka_ffmpeg::{CodecContext, Encoding, Frame, OutputContext, Packet, Raster, Sws};

use crate::convert::carry::{self, new_stream, output_time_base, shift};
use crate::convert::target::VideoTarget;
use crate::encode::{
    EncoderBackend, Family, RateControl, VideoCodec, apply_quality, find_encoder, pace,
};
use crate::error::{Error, Result, check};
use rikka_video_core::color;
use rikka_video_core::decode::{Decoded, Decoder};
use rikka_video_core::sw::fitted;

const DEEP: [&str; 13] = [
    "p10", "p12", "p14", "p16", "10le", "12le", "14le", "16le", "10be", "12be", "16be", "48le",
    "64le",
];

pub(crate) struct VideoLane {
    decoder: Decoder,
    encoder: CodecContext,
    decoded: Frame,
    staged: Frame,
    packet: Packet,
    sws: Sws,
    size: (i32, i32),
    format: AVPixelFormat,
    matrix: i32,
    full_range: bool,
    time_base: AVRational,
    index: i32,
    to: AVRational,
    shift: i64,
    last: Option<i64>,
}

impl VideoLane {
    pub(crate) fn open(
        stream: &AVStream,
        output: &OutputContext,
        target: &VideoTarget,
        start_us: i64,
    ) -> Result<VideoLane> {
        let parameters = unsafe { &*stream.codecpar };
        let decoding = unsafe { avcodec::avcodec_find_decoder(parameters.codec_id) };
        if decoding.is_null() {
            return Err(Error::Unsupported(
                "FFmpeg has no decoder for the video stream".into(),
            ));
        }
        let decoder = Decoder::open(decoding, stream.codecpar, stream.time_base, None)?;

        let (width, height) = even(fitted(
            (
                parameters.width.max(2) as u32,
                parameters.height.max(2) as u32,
            ),
            target.bounds,
        ));
        let source_format = parameters.format;
        let full_range = parameters.color_range == avutil::AVCOL_RANGE_JPEG
            || matches!(
                source_format,
                avutil::AV_PIX_FMT_YUVJ420P
                    | avutil::AV_PIX_FMT_YUVJ422P
                    | avutil::AV_PIX_FMT_YUVJ444P
            );
        let matrix = color::matrix(parameters.color_space, height as i32);
        let deep = target.codec != VideoCodec::H264 && is_deep(source_format);

        let mut last_error = None;
        for encoder in EncoderBackend::Software.encoders(target.codec) {
            let Some(codec) = find_encoder(encoder.name) else {
                continue;
            };
            let attempt = open_encoder(
                codec,
                encoder.family,
                Shape {
                    width,
                    height,
                    deep,
                    matrix,
                    full_range,
                },
                stream,
                output,
                target,
            );
            match attempt {
                Ok((context, format)) => {
                    let created = new_stream(output)?;
                    let index = unsafe {
                        let copied = avcodec::avcodec_parameters_from_context(
                            (*created).codecpar,
                            context.as_ptr(),
                        );
                        check("avcodec_parameters_from_context(video)", copied)?;
                        carry::describe(created, stream)?;
                        carry::presentation(stream.codecpar, (*created).codecpar);
                        carry::tag_for(output, created);
                        (*created).index
                    };
                    return Ok(VideoLane {
                        decoder,
                        encoder: context,
                        decoded: Frame::new()?,
                        staged: Frame::new()?,
                        packet: Packet::new()?,
                        sws: Sws::bicubic(),
                        size: (width as i32, height as i32),
                        format,
                        matrix,
                        full_range,
                        time_base: stream.time_base,
                        index,
                        to: stream.time_base,
                        shift: shift(start_us, stream.time_base),
                        last: None,
                    });
                }
                Err(error) => {
                    tracing::debug!("video encoder {} unavailable: {error}", encoder.name);
                    last_error = Some(error);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| {
            Error::Unsupported(format!(
                "no software {} encoder is compiled into this FFmpeg",
                target.codec.stream_name()
            ))
        }))
    }

    pub(crate) fn update_time_base(&mut self, output: &OutputContext) {
        self.to = output_time_base(output, self.index);
    }

    pub(crate) fn feed(
        &mut self,
        packet: Option<&mut Packet>,
        output: &mut OutputContext,
    ) -> Result<()> {
        let ending = packet.is_none();
        if let Err(error) = self.decoder.send(packet) {
            if ending {
                return Err(error.into());
            }
            tracing::debug!("skipping corrupt video packet: {error}");
            return Ok(());
        }
        loop {
            match self.decoder.receive(&mut self.decoded)? {
                Decoded::Frame => self.encode_decoded(output)?,
                Decoded::NeedsInput => return Ok(()),
                Decoded::Eof => return self.send(output, ptr::null()),
            }
        }
    }

    fn encode_decoded(&mut self, output: &mut OutputContext) -> Result<()> {
        let stamp = match self.decoded.timestamp() {
            Some(stamp) => stamp - self.shift,
            None => self
                .last
                .map_or(0, |last| last + self.decoded.span().max(1)),
        };
        if self.last.is_some_and(|last| stamp <= last) {
            self.decoded.unref();
            return Ok(());
        }
        self.last = Some(stamp);

        let passes = self.decoded.format() == self.format
            && (self.decoded.width(), self.decoded.height()) == self.size;
        let frame = match passes {
            true => {
                unsafe { (*self.decoded.as_mut_ptr()).pts = stamp };
                self.decoded.as_ptr()
            }
            false => {
                self.stage(stamp)?;
                self.staged.as_ptr()
            }
        };
        unsafe { (*frame.cast_mut()).pict_type = avutil::AV_PICTURE_TYPE_NONE };
        let sent = self.send(output, frame);
        self.decoded.unref();
        sent
    }

    fn stage(&mut self, stamp: i64) -> Result<()> {
        self.staged.unref();
        unsafe {
            let staged = self.staged.as_mut_ptr();
            (*staged).format = self.format;
            (*staged).width = self.size.0;
            (*staged).height = self.size.1;
            let allocated = avutil::av_frame_get_buffer(staged, 0);
            check("av_frame_get_buffer(video)", allocated)?;
            (*staged).pts = stamp;
            (*staged).colorspace = self.matrix;
            (*staged).color_range = match self.full_range {
                true => avutil::AVCOL_RANGE_JPEG,
                false => avutil::AVCOL_RANGE_MPEG,
            };
            (*staged).sample_aspect_ratio = (*self.encoder.as_ptr()).sample_aspect_ratio;
        }
        self.sws.fit(
            Raster {
                width: self.decoded.width(),
                height: self.decoded.height(),
                format: self.decoded.format(),
            },
            Raster {
                width: self.size.0,
                height: self.size.1,
                format: self.format,
            },
        )?;
        let source = Encoding {
            matrix: color::matrix(self.decoded.colorspace(), self.decoded.height()),
            full_range: self.full_range,
        };
        let destination = Encoding {
            matrix: self.matrix,
            full_range: self.full_range,
        };
        self.sws.describe(source, destination)?;
        unsafe {
            self.sws.scale(
                (
                    self.decoded.data().as_ptr().cast::<*const u8>(),
                    self.decoded.linesize().as_ptr(),
                    self.decoded.height(),
                ),
                (
                    self.staged.data().as_ptr(),
                    self.staged.linesize().as_ptr(),
                    self.size.1,
                ),
            )?;
        }
        Ok(())
    }

    fn send(&mut self, output: &mut OutputContext, frame: *const avutil::AVFrame) -> Result<()> {
        let (from, to, index) = (self.time_base, self.to, self.index);
        crate::encode::exchange(&self.encoder, frame, &mut self.packet, |packet| {
            packet.rescale(from, to);
            packet.set_stream_index(index);
            Ok(output.write(packet, "av_interleaved_write_frame(video)")?)
        })
    }
}

#[derive(Clone, Copy)]
struct Shape {
    width: u32,
    height: u32,
    deep: bool,
    matrix: i32,
    full_range: bool,
}

fn open_encoder(
    codec: *const AVCodec,
    family: Option<Family>,
    shape: Shape,
    stream: &AVStream,
    output: &OutputContext,
    target: &VideoTarget,
) -> Result<(CodecContext, AVPixelFormat)> {
    let context = unsafe { CodecContext::alloc(codec) }?;
    let format = pixel_format(&context, shape.deep);
    let raw = context.as_ptr();
    let parameters = unsafe { &*stream.codecpar };
    unsafe {
        (*raw).width = shape.width as i32;
        (*raw).height = shape.height as i32;
        (*raw).pix_fmt = format;
        (*raw).time_base = stream.time_base;
        (*raw).framerate = frame_rate(stream);
        (*raw).sample_aspect_ratio = sample_aspect(stream);
        (*raw).color_primaries = parameters.color_primaries;
        (*raw).color_trc = parameters.color_trc;
        (*raw).colorspace = shape.matrix;
        (*raw).color_range = match shape.full_range {
            true => avutil::AVCOL_RANGE_JPEG,
            false => avutil::AVCOL_RANGE_MPEG,
        };
        (*raw).chroma_sample_location = parameters.chroma_location;
        (*raw).thread_count = 0;
        if output.wants_global_header() {
            (*raw).flags |= avcodec::AV_CODEC_FLAG_GLOBAL_HEADER;
        }
    }
    match (family, target.rate) {
        (Some(family), RateControl::Quality(level)) => {
            apply_quality(raw, family, level);
            pace(raw, family, target.speed);
        }
        (Some(family), RateControl::Bitrate(bitrate)) => {
            unsafe { (*raw).bit_rate = bitrate as i64 };
            pace(raw, family, target.speed);
        }
        (Some(family), RateControl::Default) => pace(raw, family, target.speed),
        (None, RateControl::Bitrate(bitrate)) => unsafe { (*raw).bit_rate = bitrate as i64 },
        (None, _) => {}
    }
    let status = unsafe { avcodec::avcodec_open2(raw, codec, ptr::null_mut()) };
    check("avcodec_open2(video)", status)?;
    Ok((context, format))
}

fn pixel_format(context: &CodecContext, deep: bool) -> AVPixelFormat {
    let Some(offered) = context.pixel_formats() else {
        return avutil::AV_PIX_FMT_YUV420P;
    };
    let preferred: &[AVPixelFormat] = match deep {
        true => &[avutil::AV_PIX_FMT_YUV420P10LE, avutil::AV_PIX_FMT_YUV420P],
        false => &[avutil::AV_PIX_FMT_YUV420P],
    };
    preferred
        .iter()
        .copied()
        .find(|format| offered.contains(format))
        .or_else(|| offered.first().copied())
        .unwrap_or(avutil::AV_PIX_FMT_YUV420P)
}

fn is_deep(format: AVPixelFormat) -> bool {
    let name = unsafe { avutil::av_get_pix_fmt_name(format) };
    if name.is_null() {
        return false;
    }
    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    DEEP.iter().any(|marker| name.contains(marker))
}

fn even((width, height): (u32, u32)) -> (u32, u32) {
    ((width & !1).max(2), (height & !1).max(2))
}

fn frame_rate(stream: &AVStream) -> AVRational {
    match stream.avg_frame_rate.num > 0 && stream.avg_frame_rate.den > 0 {
        true => stream.avg_frame_rate,
        false => stream.r_frame_rate,
    }
}

fn sample_aspect(stream: &AVStream) -> AVRational {
    let parameters = unsafe { &*stream.codecpar };
    match parameters.sample_aspect_ratio.num > 0 {
        true => parameters.sample_aspect_ratio,
        false => stream.sample_aspect_ratio,
    }
}
