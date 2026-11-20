use std::ptr;
use std::time::Duration;

use rikka_ffmpeg::avcodec::{self, AVCodec, AVCodecContext};
use rikka_ffmpeg::avutil::{self, AVFrame, AVHWFramesContext, AVPixelFormat, AVRational};
use rikka_ffmpeg::{BufferRef, CodecContext, Frame, Packet};
use rikka_gpu::Gpu;
use rikka_video_core::Picture;

use crate::coded::{MICROS, MUXED, StreamInfo, from_packet};
use crate::encode::backend::{Encoder, EncoderBackend, find_encoder};
use crate::encode::config::{Latency, ParameterSets, RateControl, Spec, VideoCodec};
use crate::encode::frames::Frames;
use crate::encode::tuning;
use crate::error::{Error, Result, check};
use rikka_coded::Coded;

const SURFACES: i32 = 8;

pub(crate) struct FfmpegCoder {
    codec: CodecContext,
    frames: Frames,
    packet: Packet,
    encoder: Encoder,
    backend: EncoderBackend,
    spec: Spec,
    time_base: AVRational,
    last_place: Option<i64>,
    reopen_due: bool,
}

impl FfmpegCoder {
    pub(crate) fn open(spec: &Spec, gpu: Option<&Gpu>) -> Result<FfmpegCoder> {
        let vendor = gpu.map(Gpu::vendor_id);
        let mut last_error = None;
        for backend in spec.backend.candidates(vendor) {
            let encoders = backend.encoders(spec.codec);
            if encoders.is_empty() {
                last_error = Some(Error::Unsupported(format!(
                    "{backend:?} has no encoder for {:?} pictures in memory",
                    spec.codec
                )));
            }
            for encoder in encoders {
                let Some(codec) = find_encoder(encoder.name) else {
                    last_error = Some(Error::Unsupported(format!(
                        "encoder {} is not compiled into this FFmpeg",
                        encoder.name
                    )));
                    continue;
                };
                match FfmpegCoder::open_one(spec, backend, *encoder, codec) {
                    Ok(coder) => return Ok(coder),
                    Err(error) => {
                        tracing::debug!("encoder {} unavailable: {error}", encoder.name);
                        last_error = Some(error);
                    }
                }
            }
        }
        Err(last_error.unwrap_or_else(|| {
            Error::Unsupported(format!(
                "no working encoder for {:?} via {:?}",
                spec.codec, spec.backend
            ))
        }))
    }

    fn open_one(
        spec: &Spec,
        backend: EncoderBackend,
        encoder: Encoder,
        codec: *const AVCodec,
    ) -> Result<FfmpegCoder> {
        let time_base = time_base(spec);
        let (context, format, upload) = open_context(spec, backend, encoder, codec, time_base)?;
        Ok(FfmpegCoder {
            codec: context,
            frames: Frames::new((spec.width, spec.height), format, upload)?,
            packet: Packet::new()?,
            encoder,
            backend,
            spec: *spec,
            time_base,
            last_place: None,
            reopen_due: false,
        })
    }

    pub(crate) fn name(&self) -> &'static str {
        self.encoder.name
    }

    pub(crate) fn backend(&self) -> EncoderBackend {
        self.backend
    }

    pub(crate) fn stream(&self) -> Result<StreamInfo> {
        let (numerator, denominator) = self.spec.frame_rate;
        let muxed = match self.spec.latency {
            Latency::Buffered => self.time_base,
            Latency::Immediate => MUXED,
        };
        StreamInfo::from_context(
            self.codec.as_ptr(),
            muxed,
            Some(AVRational {
                num: numerator as i32,
                den: denominator as i32,
            }),
        )
    }

    pub(crate) fn set_bitrate(&mut self, bitrate: u64) {
        if self.spec.rate == RateControl::Bitrate(bitrate) {
            return;
        }
        self.spec.rate = RateControl::Bitrate(bitrate);
        match tuning::retunes_in_place(self.encoder) {
            true => tuning::rate(self.codec.as_ptr(), &self.spec),
            false => self.reopen_due = true,
        }
    }

    pub(crate) fn encode(
        &mut self,
        picture: Picture<'_>,
        at: Duration,
        keyframe: bool,
        coded: &mut Vec<Coded>,
    ) -> Result<()> {
        self.reopen()?;
        let place = self.place(at);
        let frame = self.frames.prepare(picture, place, keyframe)?;
        let sent = Frame::as_ptr(frame);
        self.exchange(sent, coded)
    }

    pub(crate) fn finish(&mut self, coded: &mut Vec<Coded>) -> Result<()> {
        self.exchange(ptr::null(), coded)
    }

    fn place(&mut self, at: Duration) -> i64 {
        let micros = at.as_micros().min(i64::MAX as u128) as i64;
        let place = unsafe { avutil::av_rescale_q(micros, MICROS, self.time_base) };
        let place = match self.last_place {
            Some(last) if place <= last => last + 1,
            _ => place,
        };
        self.last_place = Some(place);
        place
    }

    fn exchange(&mut self, frame: *const AVFrame, coded: &mut Vec<Coded>) -> Result<()> {
        let (time_base, codec, interval) =
            (self.time_base, self.spec.codec, self.spec.frame_interval());
        super::exchange(&self.codec, frame, &mut self.packet, |packet| {
            if let Some(mut made) = from_packet(packet, time_base) {
                if made.duration_us <= 0 {
                    made.duration_us = interval.as_micros() as i64;
                }
                coded.push(without_filler(made, codec));
            }
            Ok(())
        })
    }

    fn reopen(&mut self) -> Result<()> {
        if !std::mem::take(&mut self.reopen_due) {
            return Ok(());
        }
        let codec = find_encoder(self.encoder.name)
            .ok_or_else(|| Error::Unsupported(format!("encoder {} vanished", self.encoder.name)))?;
        let (context, format, upload) = open_context(
            &self.spec,
            self.backend,
            self.encoder,
            codec,
            self.time_base,
        )?;
        self.codec = context;
        self.frames = Frames::new((self.spec.width, self.spec.height), format, upload)?;
        tracing::debug!(
            rate = ?self.spec.rate,
            "reopened {} for a new bitrate",
            self.encoder.name
        );
        Ok(())
    }
}

impl std::fmt::Debug for FfmpegCoder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FfmpegCoder")
            .field("name", &self.encoder.name)
            .field("backend", &self.backend)
            .finish_non_exhaustive()
    }
}

fn time_base(spec: &Spec) -> AVRational {
    match spec.latency {
        Latency::Buffered => AVRational {
            num: spec.frame_rate.1 as i32,
            den: spec.frame_rate.0 as i32,
        },
        Latency::Immediate => MICROS,
    }
}

fn without_filler(coded: Coded, codec: VideoCodec) -> Coded {
    let syntax = match codec {
        VideoCodec::H264 => rikka_annexb::NalSyntax::H264,
        VideoCodec::Hevc => rikka_annexb::NalSyntax::H265,
        VideoCodec::Av1 | VideoCodec::Vp9 => return coded,
    };
    let mut data = Vec::with_capacity(coded.data.len());
    rikka_annexb::append_without_filler(&coded.data, syntax, &mut data);
    Coded {
        data: data.into(),
        ..coded
    }
}

fn open_context(
    spec: &Spec,
    backend: EncoderBackend,
    encoder: Encoder,
    codec: *const AVCodec,
    time_base: AVRational,
) -> Result<(CodecContext, AVPixelFormat, Option<BufferRef>)> {
    let context = unsafe { CodecContext::alloc(codec) }?;
    let raw = context.as_ptr();
    unsafe {
        (*raw).width = spec.width as i32;
        (*raw).height = spec.height as i32;
        (*raw).time_base = time_base;
        (*raw).framerate = AVRational {
            num: spec.frame_rate.0 as i32,
            den: spec.frame_rate.1 as i32,
        };
        (*raw).colorspace = avutil::AVCOL_SPC_BT709;
        (*raw).color_primaries = avutil::AVCOL_PRI_BT709;
        (*raw).color_trc = avutil::AVCOL_TRC_BT709;
        (*raw).color_range = avutil::AVCOL_RANGE_MPEG;
        if spec.parameter_sets == ParameterSets::OutOfBand {
            (*raw).flags |= avcodec::AV_CODEC_FLAG_GLOBAL_HEADER;
        }
    }
    let (format, upload) = surfaces(raw, backend, (spec.width, spec.height))?;
    tuning::apply(raw, encoder, spec);
    let status = unsafe { avcodec::avcodec_open2(raw, codec, ptr::null_mut()) };
    check("avcodec_open2", status)?;
    Ok((context, format, upload))
}

fn surfaces(
    raw: *mut AVCodecContext,
    backend: EncoderBackend,
    size: (u32, u32),
) -> Result<(AVPixelFormat, Option<BufferRef>)> {
    match backend {
        EncoderBackend::Software => {
            unsafe {
                (*raw).pix_fmt = avutil::AV_PIX_FMT_YUV420P;
                (*raw).thread_count = 0;
            }
            Ok((avutil::AV_PIX_FMT_YUV420P, None))
        }
        EncoderBackend::Nvenc | EncoderBackend::Amf | EncoderBackend::Qsv => {
            unsafe { (*raw).pix_fmt = avutil::AV_PIX_FMT_NV12 };
            Ok((avutil::AV_PIX_FMT_NV12, None))
        }
        EncoderBackend::Vaapi => {
            let frames = vaapi_surfaces(size)?;
            unsafe {
                (*raw).pix_fmt = avutil::AV_PIX_FMT_VAAPI;
                (*raw).hw_frames_ctx = frames.share()?.into_raw();
            }
            Ok((avutil::AV_PIX_FMT_NV12, Some(frames)))
        }
        EncoderBackend::VulkanVideo => Err(Error::Unsupported(
            "Vulkan Video encodes dmabufs, not pictures in memory".into(),
        )),
    }
}

fn vaapi_surfaces((width, height): (u32, u32)) -> Result<BufferRef> {
    let mut device = ptr::null_mut();
    let created = unsafe {
        avutil::av_hwdevice_ctx_create(
            &mut device,
            avutil::AV_HWDEVICE_TYPE_VAAPI,
            ptr::null(),
            ptr::null_mut(),
            0,
        )
    };
    check("av_hwdevice_ctx_create(vaapi)", created)?;
    let device = BufferRef::from_raw(device, "av_hwdevice_ctx_create(vaapi)")?;
    let frames = BufferRef::from_raw(
        unsafe { avutil::av_hwframe_ctx_alloc(device.as_ptr()) },
        "av_hwframe_ctx_alloc",
    )?;
    unsafe {
        let context = (*frames.as_ptr()).data.cast::<AVHWFramesContext>();
        (*context).format = avutil::AV_PIX_FMT_VAAPI;
        (*context).sw_format = avutil::AV_PIX_FMT_NV12;
        (*context).width = width as i32;
        (*context).height = height as i32;
        (*context).initial_pool_size = SURFACES;
    }
    let initialized = unsafe { avutil::av_hwframe_ctx_init(frames.as_ptr()) };
    check("av_hwframe_ctx_init", initialized)?;
    Ok(frames)
}
