mod backend;
mod config;
mod ffmpeg;
mod frames;
mod quality;
mod sound;
mod tuning;
#[cfg(unix)]
mod vulkan;

use std::path::Path;
use std::time::Duration;

use rikka_ffmpeg::avcodec;
use rikka_ffmpeg::avutil::{self, AVFrame};
use rikka_ffmpeg::{CodecContext, OutputContext, Packet};
#[cfg(unix)]
use rikka_gpu::DmabufSpec;
use rikka_gpu::Gpu;
use rikka_video_core::init;
use rikka_video_core::{Picture, PixelLayout};

use crate::audio::AudioTrack;
use crate::coded::{VideoPackets, add_stream, write_sample};
use crate::error::{Error, Result, check};
use rikka_coded::Coded;

pub(crate) use backend::find_encoder;
pub use backend::{EncoderBackend, EncoderChoice};
pub use config::{EncoderConfig, Feed, KeyframeSpacing, ParameterSets, RateControl, VideoCodec};
pub(crate) use config::{Latency, Spec};
pub(crate) use ffmpeg::FfmpegCoder;
pub use quality::EncodeSpeed;
pub(crate) use quality::{Family, apply as apply_quality, pace, set as set_option};
#[cfg(unix)]
pub use rikka_vulkan_video::Imported;
pub use sound::AudioEncoder;
#[cfg(unix)]
pub(crate) use vulkan::VulkanCoder;

const VIDEO_STREAM: usize = 0;

#[derive(Clone, Copy, Debug)]
pub struct EncodedVideo {
    pub frames: u64,
    pub duration: Duration,
    pub audio: bool,
}

pub struct VideoEncoder {
    video: VideoPackets,
    audio: Option<AudioTrack>,
    output: OutputContext,
    packet: Packet,
    config: EncoderConfig,
    frame_index: i64,
    written: u64,
}

impl VideoEncoder {
    pub fn new(gpu: Option<&Gpu>, path: impl AsRef<Path>, config: EncoderConfig) -> Result<Self> {
        init::ensure()?;
        let mut output = OutputContext::create(path.as_ref())?;
        let parameter_sets = match output.wants_global_header() {
            true => ParameterSets::OutOfBand,
            false => ParameterSets::InBand,
        };
        let video = VideoPackets::with_spec(gpu, config.spec(parameter_sets))?;
        add_stream(&output, &video.stream()?)?;
        let audio = config
            .audio
            .map(|audio| AudioTrack::open(&output, audio))
            .transpose()?;
        output.write_header(config.fast_start)?;
        tracing::info!(
            "encoding {}x{} with {} ({:?})",
            config.width,
            config.height,
            video.name(),
            video.backend(),
        );
        Ok(Self {
            video,
            audio,
            output,
            packet: Packet::new()?,
            config,
            frame_index: 0,
            written: 0,
        })
    }

    #[must_use]
    pub fn backend(&self) -> EncoderBackend {
        self.video.backend()
    }

    #[cfg(unix)]
    pub fn import(&self, spec: &DmabufSpec<'_>) -> Result<Imported> {
        self.video.import(spec)
    }

    pub fn push_frame(&mut self, rgba: &[u8]) -> Result<()> {
        let (width, height) = (self.config.width, self.config.height);
        let expected = width as usize * height as usize * 4;
        if rgba.len() != expected {
            return Err(Error::Unsupported(format!(
                "frame data is {} bytes, expected {expected} ({width}x{height} RGBA8)",
                rgba.len(),
            )));
        }
        let picture = Picture {
            bytes: rgba,
            width,
            height,
            stride: width as usize * 4,
            layout: PixelLayout::Rgba,
        };
        let place = self.frame_index;
        let coded = self.video.encode(picture, self.slot_time(place))?;
        self.wrote(place, coded)
    }

    pub fn push_frame_at(&mut self, picture: Picture<'_>, at: Duration) -> Result<bool> {
        let Some(place) = self.slot(at) else {
            return Ok(false);
        };
        let coded = self.video.encode(picture, self.slot_time(place))?;
        self.wrote(place, coded)?;
        Ok(true)
    }

    #[cfg(unix)]
    pub fn push_imported_at(&mut self, source: &Imported, at: Duration) -> Result<bool> {
        let Some(place) = self.slot(at) else {
            return Ok(false);
        };
        let coded = self.video.encode_imported(source, self.slot_time(place))?;
        self.wrote(place, coded)?;
        Ok(true)
    }

    #[must_use]
    pub fn next_frame_at(&self) -> Duration {
        self.slot_time(self.frame_index)
    }

    #[must_use]
    pub fn frames(&self) -> u64 {
        self.written
    }

    pub fn push_audio(&mut self, samples: &[f32]) -> Result<()> {
        let track = self
            .audio
            .as_mut()
            .ok_or_else(|| Error::Unsupported("this encode has no audio track".into()))?;
        track.push(&mut self.output, samples)
    }

    pub fn finish(mut self) -> Result<EncodedVideo> {
        let audio = self.audio.is_some();
        if let Some(mut track) = self.audio.take() {
            track.finish(&mut self.output)?;
        }
        let VideoEncoder {
            video,
            mut output,
            mut packet,
            config,
            frame_index,
            written,
            ..
        } = self;
        for coded in video.finish()? {
            write_sample(&mut output, &mut packet, VIDEO_STREAM, coded.sample())?;
        }
        output.write_trailer()?;
        Ok(EncodedVideo {
            frames: written,
            duration: slot_time(config.frame_rate, frame_index),
            audio,
        })
    }

    fn slot(&self, at: Duration) -> Option<i64> {
        let (numerator, denominator) = self.config.frame_rate;
        let place = (at.as_secs_f64() * f64::from(numerator) / f64::from(denominator.max(1)))
            .floor()
            .clamp(0.0, i64::MAX as f64) as i64;
        (place >= self.frame_index).then_some(place)
    }

    fn slot_time(&self, place: i64) -> Duration {
        slot_time(self.config.frame_rate, place)
    }

    fn wrote(&mut self, place: i64, coded: Vec<Coded>) -> Result<()> {
        for packet in coded {
            write_sample(
                &mut self.output,
                &mut self.packet,
                VIDEO_STREAM,
                packet.sample(),
            )?;
        }
        self.frame_index = place + 1;
        self.written += 1;
        Ok(())
    }
}

impl std::fmt::Debug for VideoEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoEncoder")
            .field("config", &self.config)
            .field("video", &self.video)
            .field("frames", &self.written)
            .finish_non_exhaustive()
    }
}

fn slot_time((numerator, denominator): (u32, u32), place: i64) -> Duration {
    let nanos = place.max(0) as u128 * u128::from(denominator) * 1_000_000_000
        / u128::from(numerator.max(1));
    Duration::from_nanos(nanos.min(u128::from(u64::MAX)) as u64)
}

pub(crate) fn exchange(
    codec: &CodecContext,
    frame: *const AVFrame,
    packet: &mut Packet,
    mut take: impl FnMut(&mut Packet) -> Result<()>,
) -> Result<()> {
    let sent = unsafe { avcodec::avcodec_send_frame(codec.as_ptr(), frame) };
    check("avcodec_send_frame", sent)?;
    loop {
        let received =
            unsafe { avcodec::avcodec_receive_packet(codec.as_ptr(), packet.as_mut_ptr()) };
        if received == avutil::AVERROR_EAGAIN || received == avutil::AVERROR_EOF {
            return Ok(());
        }
        check("avcodec_receive_packet", received)?;
        let taken = take(packet);
        packet.unref();
        taken?;
    }
}
