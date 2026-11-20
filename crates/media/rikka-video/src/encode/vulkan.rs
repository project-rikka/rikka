use std::time::Duration;

use rikka_ffmpeg::avutil::AVRational;
use rikka_gpu::{DmabufSpec, Gpu};
use rikka_vulkan_video::{Codec, Encoder, Error as VulkanError, Imported, Rate, Request};

use crate::coded::{MUXED, StreamInfo};
use crate::encode::config::{Latency, ParameterSets, RateControl, Spec, VideoCodec};
use crate::encode::quality;
use crate::error::{Error, Result};
use rikka_coded::Coded;

const SINGLE_REFERENCE: u8 = 1;
const DEFAULT_BITS_PER_PIXEL: f64 = 0.1;
const QUALITY_BITS_PER_PIXEL: f64 = 0.3;
const IMMEDIATE_BUFFER: Duration = Duration::from_millis(500);
const BUFFERED_BUFFER: Duration = Duration::from_secs(1);

pub(crate) struct VulkanCoder {
    encoder: Encoder,
    spec: Spec,
    last_frame: Option<u32>,
}

impl VulkanCoder {
    pub(crate) fn supports(gpu: &Gpu, codec: VideoCodec) -> bool {
        vulkan_codec(codec).is_some_and(|codec| Encoder::supports(gpu, codec))
    }

    pub(crate) fn open(spec: &Spec, gpu: Option<&Gpu>) -> Result<VulkanCoder> {
        let gpu = gpu.ok_or_else(|| {
            Error::Unsupported("Vulkan Video needs a device on the dmabufs' GPU".into())
        })?;
        let codec = vulkan_codec(spec.codec).ok_or_else(|| {
            Error::Unsupported(format!("Vulkan Video does not encode {:?}", spec.codec))
        })?;
        let encoder = Encoder::builder(gpu, codec, spec.width, spec.height)
            .frame_rate(spec.nominal_rate())
            .references(SINGLE_REFERENCE)
            .rate(rate(spec))
            .build()?;
        Ok(VulkanCoder {
            encoder,
            spec: *spec,
            last_frame: None,
        })
    }

    pub(crate) fn name(&self) -> &str {
        self.encoder.name()
    }

    pub(crate) fn import(&self, spec: &DmabufSpec<'_>) -> Result<Imported> {
        Ok(self.encoder.import(spec)?)
    }

    pub(crate) fn stream(&self) -> Result<StreamInfo> {
        let (numerator, denominator) = self.spec.frame_rate;
        StreamInfo::from_parameter_sets(
            self.spec.codec,
            self.encoder.size(),
            self.encoder.parameter_sets(),
            MUXED,
            AVRational {
                num: numerator as i32,
                den: denominator as i32,
            },
        )
    }

    pub(crate) fn set_bitrate(&mut self, bitrate: u64) {
        self.spec.rate = RateControl::Bitrate(bitrate);
        self.encoder.set_rate(rate(&self.spec));
    }

    pub(crate) fn encode(
        &mut self,
        source: &Imported,
        at: Duration,
        keyframe: bool,
    ) -> Result<Coded> {
        let request = match (keyframe, self.last_frame) {
            (false, Some(last)) => Request::predicted(last),
            _ => Request::intra(),
        };
        let request = match self.spec.rate {
            RateControl::Quality(level) => request.constant(quality::coded_qp(level)),
            RateControl::Default | RateControl::Bitrate(_) => request,
        };
        let encoded = match self.encoder.encode(source, request) {
            Err(VulkanError::Evicted(frame)) => {
                tracing::debug!(frame, "the reference left the ring; coding an intra frame");
                self.encoder.encode(source, Request::intra())?
            }
            encoded => encoded?,
        };
        self.last_frame = Some(encoded.frame);
        let data = match (self.spec.parameter_sets, encoded.intra) {
            (ParameterSets::OutOfBand, true) => encoded
                .data
                .strip_prefix(self.encoder.parameter_sets())
                .map_or_else(|| encoded.data.clone(), <[u8]>::to_vec),
            _ => encoded.data,
        };
        let pts_us = at.as_micros().min(i64::MAX as u128) as i64;
        Ok(Coded {
            data: data.into(),
            pts_us,
            dts_us: pts_us,
            duration_us: self.spec.frame_interval().as_micros() as i64,
            keyframe: encoded.intra,
        })
    }
}

impl std::fmt::Debug for VulkanCoder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VulkanCoder")
            .field("encoder", &self.encoder)
            .finish_non_exhaustive()
    }
}

fn vulkan_codec(codec: VideoCodec) -> Option<Codec> {
    match codec {
        VideoCodec::H264 => Some(Codec::H264),
        VideoCodec::Hevc => Some(Codec::H265),
        VideoCodec::Av1 | VideoCodec::Vp9 => None,
    }
}

fn rate(spec: &Spec) -> Rate {
    let pixel_rate = f64::from(spec.width) * f64::from(spec.height) * f64::from(spec.frame_rate.0)
        / f64::from(spec.frame_rate.1);
    let bitrate = match spec.rate {
        RateControl::Bitrate(bitrate) => bitrate,
        RateControl::Default => (pixel_rate * DEFAULT_BITS_PER_PIXEL) as u64,
        RateControl::Quality(level) => {
            (pixel_rate * QUALITY_BITS_PER_PIXEL * f64::from(level) / 100.0) as u64
        }
    };
    let buffer = match spec.latency {
        Latency::Immediate => IMMEDIATE_BUFFER,
        Latency::Buffered => BUFFERED_BUFFER,
    };
    Rate {
        bitrate: bitrate.max(1),
        buffer,
        ..Rate::default()
    }
}
