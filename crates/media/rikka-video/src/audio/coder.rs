use std::ffi::CString;
use std::ptr;

use rikka_ffmpeg::avcodec::{self, AVCodec, AVCodecContext, AVSampleFormat};
use rikka_ffmpeg::avutil::{self, AVFrame, AVRational};
use rikka_ffmpeg::{CodecContext, Frame, Packet};
use rikka_resample::{ResampleQuality, Resampler};

use crate::audio::fold::Fold;
use crate::audio::samples::{Layout, Quantizer};
use crate::audio::{AudioCodec, AudioConfig};
use crate::encode::{EncodeSpeed, RateControl};
use crate::error::{Error, Result, check};

const DEFAULT_FRAME_SIZE: usize = 1024;
const MAX_CHANNELS: u16 = 8;

pub(crate) type Emit<'a> = dyn FnMut(&mut Packet) -> Result<()> + 'a;

pub(crate) struct SoundCoder {
    codec_ctx: CodecContext,
    frame: Frame,
    packet: Packet,
    time_base: AVRational,
    layout: Layout,
    fold: Fold,
    resampler: Option<Resampler>,
    quantizer: Quantizer,
    folded: Vec<f32>,
    pending: Vec<f32>,
    frame_size: usize,
    pads_last_frame: bool,
    channels: usize,
    next_pts: i64,
}

impl SoundCoder {
    pub(crate) fn open(config: AudioConfig, global_header: bool) -> Result<Self> {
        if config.channels() == 0 || config.channels() > MAX_CHANNELS {
            return Err(Error::Unsupported(format!(
                "audio channel count {} is not encodable",
                config.channels()
            )));
        }
        if config.sample_rate() == 0 {
            return Err(Error::Unsupported("audio at 0 Hz is not encodable".into()));
        }
        let (codec, name) = encoder(config.encoded_as())?;
        let context = unsafe { CodecContext::alloc(codec) }?;
        let channels = channels_for(&context, config.channels());
        let rate = rate_for(&context, config.sample_rate());
        let layout = layout_for(&context, &config);
        let time_base = AVRational {
            num: 1,
            den: rate as i32,
        };

        let raw = context.as_ptr();
        unsafe {
            (*raw).sample_fmt = layout.format;
            (*raw).sample_rate = rate as i32;
            (*raw).time_base = time_base;
            avutil::av_channel_layout_default(&mut (*raw).ch_layout, channels as i32);
            if matches!(
                layout.format,
                avcodec::AV_SAMPLE_FMT_S32 | avcodec::AV_SAMPLE_FMT_S32P
            ) {
                (*raw).bits_per_raw_sample = layout.depth.min(32) as i32;
            }
            if global_header {
                (*raw).flags |= avcodec::AV_CODEC_FLAG_GLOBAL_HEADER;
            }
        }
        rate_control(raw, &config, channels);

        let status = unsafe { avcodec::avcodec_open2(raw, codec, ptr::null_mut()) };
        check(name, status)?;

        let (frame_size, capabilities) = unsafe {
            let frame_size = (*raw).frame_size;
            (
                match frame_size > 0 {
                    true => frame_size as usize,
                    false => DEFAULT_FRAME_SIZE,
                },
                (*codec).capabilities,
            )
        };
        let short_last =
            avcodec::AV_CODEC_CAP_SMALL_LAST_FRAME | avcodec::AV_CODEC_CAP_VARIABLE_FRAME_SIZE;

        Ok(Self {
            codec_ctx: context,
            frame: Frame::new()?,
            packet: Packet::new()?,
            time_base,
            layout,
            fold: Fold::new(usize::from(config.channels()), usize::from(channels)),
            resampler: (rate != config.sample_rate()).then(|| {
                Resampler::with_quality(ResampleQuality::High, config.sample_rate(), rate, channels)
            }),
            quantizer: Quantizer::new(),
            folded: Vec::new(),
            pending: Vec::new(),
            frame_size,
            pads_last_frame: capabilities & short_last == 0,
            channels: usize::from(channels),
            next_pts: 0,
        })
    }

    pub(crate) fn context(&self) -> *mut AVCodecContext {
        self.codec_ctx.as_ptr()
    }

    pub(crate) fn time_base(&self) -> AVRational {
        self.time_base
    }

    pub(crate) fn push(&mut self, samples: &[f32], emit: &mut Emit<'_>) -> Result<()> {
        let incoming = self.fold.input_channels();
        if !samples.len().is_multiple_of(incoming) {
            return Err(Error::Unsupported(format!(
                "{} samples is not a whole number of {incoming}-channel frames",
                samples.len(),
            )));
        }
        self.folded.clear();
        self.fold.apply(samples, &mut self.folded);
        match &mut self.resampler {
            Some(resampler) => resampler.process(&self.folded, &mut self.pending),
            None => self.pending.extend_from_slice(&self.folded),
        }
        self.drain(emit)
    }

    pub(crate) fn finish(&mut self, emit: &mut Emit<'_>) -> Result<()> {
        if let Some(resampler) = &mut self.resampler {
            resampler.flush(&mut self.pending);
        }
        self.drain(emit)?;
        if !self.pending.is_empty() {
            if self.pads_last_frame {
                self.pending.resize(self.frame_size * self.channels, 0.0);
            }
            let block = 0..self.pending.len();
            self.encode_block(block, emit)?;
            self.pending.clear();
        }
        self.send(ptr::null(), emit)
    }

    fn drain(&mut self, emit: &mut Emit<'_>) -> Result<()> {
        let per_frame = self.frame_size * self.channels;
        let mut consumed = 0;
        while self.pending.len() - consumed >= per_frame {
            self.encode_block(consumed..consumed + per_frame, emit)?;
            consumed += per_frame;
        }
        self.pending.drain(..consumed);
        Ok(())
    }

    fn encode_block(&mut self, block: std::ops::Range<usize>, emit: &mut Emit<'_>) -> Result<()> {
        let samples_per_channel = (block.end - block.start) / self.channels;
        self.frame.unref();
        unsafe {
            let frame = self.frame.as_mut_ptr();
            (*frame).format = self.layout.format;
            (*frame).nb_samples = samples_per_channel as i32;
            (*frame).sample_rate = self.time_base.den;
            avutil::av_channel_layout_default(&mut (*frame).ch_layout, self.channels as i32);
            let allocated = avutil::av_frame_get_buffer(frame, 0);
            check("av_frame_get_buffer(audio)", allocated)?;
            (*frame).pts = self.next_pts;
        }

        let planes = *self.frame.data();
        self.quantizer
            .fill(self.layout, &planes, self.channels, &self.pending[block]);

        self.next_pts += samples_per_channel as i64;
        self.send(self.frame.as_ptr(), emit)
    }

    fn send(&mut self, frame_ptr: *const AVFrame, emit: &mut Emit<'_>) -> Result<()> {
        crate::encode::exchange(&self.codec_ctx, frame_ptr, &mut self.packet, emit)
    }
}

impl std::fmt::Debug for SoundCoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundCoder")
            .field("channels", &self.channels)
            .field("frame_size", &self.frame_size)
            .field("samples", &self.next_pts)
            .finish_non_exhaustive()
    }
}

fn encoder(codec: AudioCodec) -> Result<(*const AVCodec, &'static str)> {
    codec
        .encoders()
        .iter()
        .find_map(|name| {
            let named = CString::new(*name).ok()?;
            let found = unsafe { avcodec::avcodec_find_encoder_by_name(named.as_ptr()) };
            (!found.is_null()).then_some((found, *name))
        })
        .ok_or_else(|| {
            Error::Unsupported(format!(
                "no {} encoder is compiled into this FFmpeg",
                codec.stream_name()
            ))
        })
}

fn channels_for(context: &CodecContext, requested: u16) -> u16 {
    let Some(counts) = context.channel_counts() else {
        return requested;
    };
    if counts.iter().any(|count| *count == i32::from(requested)) {
        return requested;
    }
    let most = counts.iter().copied().max().unwrap_or(2).clamp(1, 2) as u16;
    requested.min(most).max(1)
}

fn rate_for(context: &CodecContext, requested: u32) -> u32 {
    let Some(mut rates) = context.sample_rates() else {
        return requested;
    };
    rates.retain(|rate| *rate > 0);
    rates.sort_unstable();
    let requested_i32 = requested as i32;
    rates
        .iter()
        .copied()
        .find(|rate| *rate >= requested_i32)
        .or_else(|| rates.last().copied())
        .map_or(requested, |rate| rate as u32)
}

fn layout_for(context: &CodecContext, config: &AudioConfig) -> Layout {
    let depth = config.depth().unwrap_or(16).clamp(8, 32);
    let lossless = config.encoded_as().is_lossless();
    let preferred: &[AVSampleFormat] = match (lossless, depth > 16) {
        (true, true) => &[
            avcodec::AV_SAMPLE_FMT_S32,
            avcodec::AV_SAMPLE_FMT_S32P,
            avcodec::AV_SAMPLE_FMT_S16,
            avcodec::AV_SAMPLE_FMT_S16P,
        ],
        (true, false) => &[
            avcodec::AV_SAMPLE_FMT_S16,
            avcodec::AV_SAMPLE_FMT_S16P,
            avcodec::AV_SAMPLE_FMT_S32,
            avcodec::AV_SAMPLE_FMT_S32P,
        ],
        (false, _) => &[
            avcodec::AV_SAMPLE_FMT_FLTP,
            avcodec::AV_SAMPLE_FMT_FLT,
            avcodec::AV_SAMPLE_FMT_S16P,
            avcodec::AV_SAMPLE_FMT_S16,
            avcodec::AV_SAMPLE_FMT_S32P,
            avcodec::AV_SAMPLE_FMT_S32,
        ],
    };
    let format = match context.sample_formats() {
        None => preferred[0],
        Some(offered) => preferred
            .iter()
            .copied()
            .find(|format| offered.contains(format))
            .or_else(|| offered.first().copied())
            .unwrap_or(preferred[0]),
    };
    let depth = match format {
        avcodec::AV_SAMPLE_FMT_S16 | avcodec::AV_SAMPLE_FMT_S16P => 16,
        avcodec::AV_SAMPLE_FMT_S32 | avcodec::AV_SAMPLE_FMT_S32P if lossless => 24,
        _ => 32,
    };
    Layout { format, depth }
}

fn rate_control(raw: *mut AVCodecContext, config: &AudioConfig, channels: u16) {
    let codec = config.encoded_as();
    let channels = u64::from(channels.max(1));
    unsafe {
        match (codec, config.rate_control()) {
            (AudioCodec::Flac | AudioCodec::Alac, _) => {}
            (_, RateControl::Bitrate(bitrate)) => (*raw).bit_rate = bitrate as i64,
            (AudioCodec::Mp3, RateControl::Quality(quality)) => {
                let level = (100 - i32::from(quality.clamp(1, 100))) * 9 / 99;
                (*raw).flags |= avcodec::AV_CODEC_FLAG_QSCALE;
                (*raw).global_quality = level * avcodec::FF_QP2LAMBDA;
            }
            (AudioCodec::Vorbis, RateControl::Quality(quality)) => {
                (*raw).flags |= avcodec::AV_CODEC_FLAG_QSCALE;
                (*raw).global_quality =
                    i32::from(quality.clamp(1, 100)) * avcodec::FF_QP2LAMBDA / 10;
            }
            (_, RateControl::Quality(quality)) => {
                (*raw).bit_rate = (codec.bitrate_per_channel(quality) * channels) as i64;
            }
            (AudioCodec::Aac, RateControl::Default) => {
                (*raw).bit_rate = (64_000 * channels) as i64;
            }
            (_, RateControl::Default) => {}
        }
        if codec == AudioCodec::Flac {
            (*raw).compression_level = match config.pace() {
                EncodeSpeed::Fastest => 0,
                EncodeSpeed::Fast => 3,
                EncodeSpeed::Balanced => 5,
                EncodeSpeed::Slow => 8,
            };
        }
    }
}
