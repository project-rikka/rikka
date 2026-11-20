use std::time::Duration;

use rikka_gpu::Gpu;
use rikka_video::{EncodeSpeed, Feed, PacketConfig, VideoPackets};

use crate::error::{Error, Result};
use crate::options::CaptureOptions;

const KEYFRAMES_EVERY: Duration = Duration::from_secs(2);

pub(crate) struct Coder {
    pub(crate) video: VideoPackets,
    pub(crate) canvas: (u32, u32),
    pub(crate) feed: Feed,
}

impl Coder {
    pub(crate) fn open(
        gpu: Option<&Gpu>,
        capture: &CaptureOptions,
        canvas: (u32, u32),
        feed: Feed,
    ) -> Result<Coder> {
        let mut refusals = Vec::new();
        for codec in capture.codecs() {
            let config = PacketConfig::new(canvas.0, canvas.1)
                .frame_rate(capture.fps)
                .quality(capture.quality)
                .codec(*codec)
                .speed(EncodeSpeed::Fast)
                .keyframe_every(KEYFRAMES_EVERY)
                .feed(feed);
            match VideoPackets::open(gpu, config) {
                Ok(video) => {
                    return Ok(Coder {
                        video,
                        canvas,
                        feed,
                    });
                }
                Err(error) => refusals.push((*codec, error)),
            }
        }
        Err(Error::NoCodec(refusals))
    }

    #[must_use]
    pub(crate) fn keyframes_every() -> Duration {
        KEYFRAMES_EVERY
    }
}
