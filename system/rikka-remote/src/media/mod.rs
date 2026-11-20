mod clipboard;
mod error;
mod input;
mod sound;
mod video;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rikka_remote_link::VideoFrame;
use rikka_remote_wire::ServerMessage;
use rikka_vulkan_video::Codec;

use clipboard::ClipboardSync;
use sound::SoundWorker;
use video::VideoWorker;

pub(crate) use error::MediaError;
pub(crate) use input::{InputCommand, InputHandle, InputWorker};
pub(crate) use video::{Recover, Size, VideoCommand, VideoControl, VideoSetup, codecs};

pub(crate) type MediaSink = tokio::sync::mpsc::UnboundedSender<MediaEvent>;

const NO_FRAME: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Viewport {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) refresh_hz: f64,
}

impl Viewport {
    pub(crate) fn from_wire(viewport: rikka_remote_wire::Viewport) -> Viewport {
        Viewport {
            width: viewport.width.max(1),
            height: viewport.height.max(1),
            refresh_hz: f64::from(viewport.refresh_mhz) / 1000.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DisplayInfo {
    pub(crate) name: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) refresh_hz: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StreamInfo {
    pub(crate) stream: u16,
    pub(crate) codec: Codec,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) frame_rate: u32,
    pub(crate) encoder: String,
}

#[derive(Debug)]
pub(crate) struct OutgoingFrame {
    pub(crate) frame: VideoFrame,
    pub(crate) abandons: Option<u32>,
    pub(crate) encode: Duration,
    pub(crate) captured: Instant,
}

#[derive(Debug)]
pub(crate) struct SoundPacket {
    pub(crate) data: Vec<u8>,
    pub(crate) samples: u64,
}

#[derive(Debug)]
pub(crate) enum MediaEvent {
    Frame(OutgoingFrame),
    Sound(SoundPacket),
    Display(DisplayInfo),
    Stream(StreamInfo),
    Message(ServerMessage),
    Pointer { x: i32, y: i32 },
    Failed(MediaError),
}

#[derive(Debug)]
pub(crate) struct Backlog {
    queued_us: AtomicU64,
    refinement: AtomicU32,
    drain_below_us: AtomicU64,
}

impl Default for Backlog {
    fn default() -> Backlog {
        Backlog {
            queued_us: AtomicU64::new(0),
            refinement: AtomicU32::new(NO_FRAME),
            drain_below_us: AtomicU64::new(0),
        }
    }
}

impl Backlog {
    pub(crate) fn queued(&self) -> Duration {
        Duration::from_micros(self.queued_us.load(Ordering::Relaxed))
    }

    pub(crate) fn set_queued(&self, queued: Duration) {
        self.queued_us
            .store(queued.as_micros() as u64, Ordering::Relaxed);
    }

    pub(crate) fn holds(&self, frame: u32) -> bool {
        self.refinement.load(Ordering::Relaxed) == frame
    }

    pub(crate) fn refinement(&self) -> Option<u32> {
        match self.refinement.load(Ordering::Relaxed) {
            NO_FRAME => None,
            frame => Some(frame),
        }
    }

    pub(crate) fn set_refinement(&self, frame: Option<u32>) {
        self.refinement
            .store(frame.unwrap_or(NO_FRAME), Ordering::Relaxed);
    }

    pub(crate) fn ask_drain(&self, below: Duration) {
        self.drain_below_us
            .store((below.as_micros() as u64).max(1), Ordering::Relaxed);
    }

    pub(crate) fn drained(&self, queued: Duration) -> bool {
        let below = self.drain_below_us.load(Ordering::Relaxed);
        below != 0
            && queued.as_micros() as u64 <= below
            && self
                .drain_below_us
                .compare_exchange(below, 0, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MediaSetup {
    pub(crate) video: VideoSetup,
    pub(crate) sound: bool,
}

pub(crate) struct Media {
    video: VideoWorker,
    sound: Option<SoundWorker>,
    clipboard: Option<ClipboardSync>,
    sink: MediaSink,
}

impl Media {
    pub(crate) fn start(
        setup: MediaSetup,
        sink: MediaSink,
        backlog: Arc<Backlog>,
    ) -> Result<Media, MediaError> {
        let video = VideoWorker::start(setup.video, sink.clone(), backlog)?;
        let clipboard = ClipboardSync::start(sink.clone())
            .inspect_err(|error| tracing::warn!("{error}"))
            .ok();
        let mut media = Media {
            video,
            sound: None,
            clipboard,
            sink,
        };
        media.set_sound(setup.sound);
        Ok(media)
    }

    pub(crate) fn video(&self) -> VideoControl {
        self.video.sender()
    }

    pub(crate) fn tell(&self, command: VideoCommand) {
        self.video.send(command);
    }

    pub(crate) fn set_sound(&mut self, on: bool) {
        if on == self.sound.is_some() {
            return;
        }
        self.sound = match on {
            true => SoundWorker::start(self.sink.clone())
                .inspect_err(|error| tracing::warn!("{error}"))
                .ok(),
            false => None,
        };
    }

    pub(crate) fn set_clipboard(&self, text: String) {
        if let Some(clipboard) = &self.clipboard {
            clipboard.set(text);
        }
    }
}
