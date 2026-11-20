use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rikka_gpu::ash::vk;
use rikka_picture::{Feed, FeedDevice};
use rikka_remote_link::{Chain, VideoFrame};
use rikka_remote_wire::{Codec, Stream};
use rikka_video::{GpuLiveDecoder, LiveCodec, LiveDecoder};
use rikka_worker::{CommandReceiver, CommandSender, Worker, command_channel};
use tokio::sync::mpsc::UnboundedSender;

const THREAD: &str = "rikka-remote-viewer-picture";
const DEVICE_PATIENCE: Duration = Duration::from_secs(5);
const MOST_QUEUED_FRAMES: usize = 4;

#[derive(Debug, thiserror::Error)]
pub(crate) enum VideoError {
    #[error("cannot start the picture thread: {0}")]
    Thread(#[from] std::io::Error),
    #[error("the picture thread stopped before it began")]
    Exited,
}

impl From<rikka_worker::Error<Infallible>> for VideoError {
    fn from(error: rikka_worker::Error<Infallible>) -> VideoError {
        match error {
            rikka_worker::Error::Spawn(error) => VideoError::Thread(error),
            rikka_worker::Error::Startup(never) => match never {},
            rikka_worker::Error::Exited => VideoError::Exited,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Undecoded {
    pub(crate) stream: u16,
    pub(crate) frame: u32,
}

enum Picture {
    Stream(Stream),
    Frame(VideoFrame),
    Stop,
}

pub(crate) struct VideoWorker {
    worker: Worker<CommandSender<Picture>>,
    queued: Arc<AtomicUsize>,
}

impl VideoWorker {
    pub(crate) fn start(
        feed: Feed,
        failures: UnboundedSender<Undecoded>,
    ) -> Result<VideoWorker, VideoError> {
        let (pictures, received) = command_channel(|| Picture::Stop)?;
        let queued = Arc::new(AtomicUsize::new(0));
        let decoded = Arc::clone(&queued);
        let worker = Worker::spawn(THREAD, pictures, move || {
            Ok::<_, Infallible>(move || decode(&feed, &received, &decoded, &failures))
        })?;
        Ok(VideoWorker { worker, queued })
    }

    pub(crate) fn begin_stream(&self, stream: Stream) {
        self.send(Picture::Stream(stream));
    }

    pub(crate) fn has_room(&self) -> bool {
        self.queued.load(Ordering::Acquire) < MOST_QUEUED_FRAMES
    }

    pub(crate) fn push(&self, frame: VideoFrame) {
        self.queued.fetch_add(1, Ordering::AcqRel);
        self.send(Picture::Frame(frame));
    }

    fn send(&self, picture: Picture) {
        if let Err(error) = self.worker.handle().send(picture) {
            tracing::debug!("the picture thread is gone: {error}");
        }
    }
}

enum Decoding {
    Gpu(GpuLiveDecoder),
    Cpu(LiveDecoder),
}

pub(crate) fn gpu_codecs(device: Option<&FeedDevice>) -> Vec<Codec> {
    let Some(FeedDevice::Video(handles)) = device else {
        return Vec::new();
    };
    let operations = handles
        .queue_families
        .iter()
        .fold(vk::VideoCodecOperationFlagsKHR::NONE, |all, family| {
            all | family.video_ops
        });
    [
        (Codec::H265, vk::VideoCodecOperationFlagsKHR::DECODE_H265),
        (Codec::H264, vk::VideoCodecOperationFlagsKHR::DECODE_H264),
        (Codec::Av1, vk::VideoCodecOperationFlagsKHR::DECODE_AV1),
    ]
    .into_iter()
    .filter(|(_, operation)| operations.contains(*operation))
    .map(|(codec, _)| codec)
    .collect()
}

fn decode(
    feed: &Feed,
    pictures: &CommandReceiver<Picture>,
    queued: &AtomicUsize,
    failures: &UnboundedSender<Undecoded>,
) {
    let mut decoding: Option<Decoding> = None;
    let mut shown = Chain::new();
    loop {
        for picture in pictures.drain() {
            match picture {
                Picture::Stream(stream) => {
                    decoding = open(feed, stream.codec);
                    if decoding.is_none() {
                        let unopened = Undecoded {
                            stream: stream.stream,
                            frame: 0,
                        };
                        if failures.send(unopened).is_err() {
                            return;
                        }
                    }
                }
                Picture::Frame(frame) => {
                    queued.fetch_sub(1, Ordering::AcqRel);
                    let Some(decoder) = &mut decoding else {
                        continue;
                    };
                    if !shown.decodable(&frame) {
                        continue;
                    }
                    match show(feed, decoder, &frame) {
                        Ok(()) => shown.decoded(&frame),
                        Err(error) => {
                            tracing::warn!(frame = frame.frame, "cannot decode a frame: {error}");
                            let failed = Undecoded {
                                stream: frame.stream,
                                frame: frame.frame,
                            };
                            if failures.send(failed).is_err() {
                                return;
                            }
                        }
                    }
                }
                Picture::Stop => return,
            }
        }
        if let Err(error) = pictures.wait() {
            tracing::warn!("the pictures stopped: {error}");
            return;
        }
    }
}

fn show(feed: &Feed, decoder: &mut Decoding, frame: &VideoFrame) -> rikka_video::Result<()> {
    match decoder {
        Decoding::Gpu(decoder) => decoder.decode(&frame.data).map(|frames| {
            for decoded in frames {
                feed.show_video(decoded.width, decoded.height, decoded.frame);
            }
        }),
        Decoding::Cpu(decoder) => decoder.decode(&frame.data, None).map(|pictures| {
            for picture in pictures {
                feed.show(picture.width, picture.height, picture.rgba);
            }
        }),
    }
}

fn open(feed: &Feed, codec: Codec) -> Option<Decoding> {
    let live = match codec {
        Codec::H264 => LiveCodec::H264,
        Codec::H265 => LiveCodec::Hevc,
        Codec::Av1 => LiveCodec::Av1,
    };
    if let Some(FeedDevice::Video(handles)) = feed.wait_for_device(DEVICE_PATIENCE) {
        match GpuLiveDecoder::open(live, &handles) {
            Ok(decoder) => {
                tracing::info!(decoder = decoder.name(), "decoding on the GPU");
                return Some(Decoding::Gpu(decoder));
            }
            Err(error) => tracing::warn!("cannot decode {codec:?} on the GPU: {error}"),
        }
    }
    match LiveDecoder::open(live) {
        Ok(decoder) => {
            tracing::info!(decoder = decoder.name(), "decoding in software");
            Some(Decoding::Cpu(decoder))
        }
        Err(error) => {
            tracing::error!("cannot decode {codec:?}: {error}");
            None
        }
    }
}
