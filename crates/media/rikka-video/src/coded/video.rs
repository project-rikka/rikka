use std::time::Duration;

#[cfg(unix)]
use rikka_gpu::DmabufSpec;
use rikka_gpu::Gpu;
use rikka_video_core::{Picture, init};

use crate::coded::config::PacketConfig;
use crate::coded::stream::StreamInfo;
use crate::encode::{EncoderBackend, Feed, FfmpegCoder, KeyframeSpacing, Spec};
#[cfg(unix)]
use crate::encode::{EncoderChoice, Imported, VideoCodec, VulkanCoder};
#[cfg(unix)]
use crate::error::Error;
use crate::error::Result;
use rikka_coded::Coded;

pub struct VideoPackets {
    coder: Coder,
    spec: Spec,
    last: Option<i64>,
    last_key: Option<i64>,
    keyframe_due: bool,
}

enum Coder {
    Ffmpeg(Box<FfmpegCoder>),
    #[cfg(unix)]
    Vulkan(Box<VulkanCoder>),
}

impl VideoPackets {
    pub fn open(gpu: Option<&Gpu>, config: PacketConfig) -> Result<VideoPackets> {
        let video = VideoPackets::with_spec(gpu, config.spec())?;
        tracing::info!(
            "coding {}x{} {:?} with {} ({:?}, {:?} feed)",
            video.spec.width,
            video.spec.height,
            video.spec.codec,
            video.name(),
            video.backend(),
            video.spec.feed,
        );
        Ok(video)
    }

    #[cfg(unix)]
    #[must_use]
    pub fn takes_dmabufs(gpu: &Gpu, codec: VideoCodec) -> bool {
        VulkanCoder::supports(gpu, codec)
    }

    pub(crate) fn with_spec(gpu: Option<&Gpu>, spec: Spec) -> Result<VideoPackets> {
        init::ensure()?;
        let spec = spec.checked()?;
        let coder = match (spec.feed, spec.backend) {
            (Feed::Memory, _) => Coder::Ffmpeg(Box::new(FfmpegCoder::open(&spec, gpu)?)),
            #[cfg(unix)]
            (
                Feed::Dmabuf,
                EncoderChoice::Auto | EncoderChoice::Only(EncoderBackend::VulkanVideo),
            ) => Coder::Vulkan(Box::new(VulkanCoder::open(&spec, gpu)?)),
            #[cfg(unix)]
            (Feed::Dmabuf, EncoderChoice::Only(other)) => {
                return Err(Error::Unsupported(format!(
                    "{other:?} cannot read dmabufs; only Vulkan Video does"
                )));
            }
        };
        Ok(VideoPackets {
            coder,
            spec,
            last: None,
            last_key: None,
            keyframe_due: true,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        match &self.coder {
            Coder::Ffmpeg(coder) => coder.name(),
            #[cfg(unix)]
            Coder::Vulkan(coder) => coder.name(),
        }
    }

    #[must_use]
    pub fn backend(&self) -> EncoderBackend {
        match &self.coder {
            Coder::Ffmpeg(coder) => coder.backend(),
            #[cfg(unix)]
            Coder::Vulkan(_) => EncoderBackend::VulkanVideo,
        }
    }

    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        (self.spec.width, self.spec.height)
    }

    pub fn request_keyframe(&mut self) {
        self.keyframe_due = true;
    }

    pub fn set_bitrate(&mut self, bits_per_second: u64) {
        match &mut self.coder {
            Coder::Ffmpeg(coder) => coder.set_bitrate(bits_per_second),
            #[cfg(unix)]
            Coder::Vulkan(coder) => coder.set_bitrate(bits_per_second),
        }
    }

    pub fn stream(&self) -> Result<StreamInfo> {
        match &self.coder {
            Coder::Ffmpeg(coder) => coder.stream(),
            #[cfg(unix)]
            Coder::Vulkan(coder) => coder.stream(),
        }
    }

    #[cfg(unix)]
    pub fn import(&self, spec: &DmabufSpec<'_>) -> Result<Imported> {
        match &self.coder {
            Coder::Vulkan(coder) => coder.import(spec),
            Coder::Ffmpeg(_) => Err(Error::Unsupported(
                "this encoder takes pictures in memory".into(),
            )),
        }
    }

    pub fn encode(&mut self, picture: Picture<'_>, at: Duration) -> Result<Vec<Coded>> {
        let (at, keyframe) = self.placed(at);
        #[cfg(unix)]
        let Coder::Ffmpeg(coder) = &mut self.coder else {
            return Err(Error::Unsupported("this encoder takes dmabufs".into()));
        };
        #[cfg(windows)]
        let Coder::Ffmpeg(coder) = &mut self.coder;
        let mut coded = Vec::new();
        coder.encode(picture, at, keyframe, &mut coded)?;
        self.keyed(&coded);
        Ok(coded)
    }

    #[cfg(unix)]
    pub fn encode_imported(&mut self, source: &Imported, at: Duration) -> Result<Vec<Coded>> {
        let (at, keyframe) = self.placed(at);
        let Coder::Vulkan(coder) = &mut self.coder else {
            return Err(Error::Unsupported(
                "this encoder takes pictures in memory".into(),
            ));
        };
        let coded = vec![coder.encode(source, at, keyframe)?];
        self.keyed(&coded);
        Ok(coded)
    }

    pub fn finish(mut self) -> Result<Vec<Coded>> {
        let mut coded = Vec::new();
        match &mut self.coder {
            Coder::Ffmpeg(coder) => coder.finish(&mut coded)?,
            #[cfg(unix)]
            Coder::Vulkan(_) => {}
        }
        Ok(coded)
    }

    fn placed(&mut self, at: Duration) -> (Duration, bool) {
        let place = at.as_micros().min(i64::MAX as u128) as i64;
        let place = match self.last {
            Some(last) if place <= last => last + 1,
            _ => place,
        };
        self.last = Some(place);
        let overdue = match self.spec.keyframes {
            KeyframeSpacing::Every(every) => self
                .last_key
                .is_none_or(|key| place - key >= every.as_micros() as i64),
            KeyframeSpacing::OnRequest => self.last_key.is_none(),
        };
        let keyframe = std::mem::take(&mut self.keyframe_due) || overdue;
        if keyframe {
            self.last_key = Some(place);
        }
        (Duration::from_micros(place as u64), keyframe)
    }

    fn keyed(&mut self, coded: &[Coded]) {
        for packet in coded.iter().filter(|packet| packet.keyframe) {
            self.last_key = Some(
                self.last_key
                    .map_or(packet.pts_us, |key| key.max(packet.pts_us)),
            );
        }
    }
}

impl std::fmt::Debug for VideoPackets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoPackets")
            .field("name", &self.name())
            .field("backend", &self.backend())
            .field("spec", &self.spec)
            .finish_non_exhaustive()
    }
}
