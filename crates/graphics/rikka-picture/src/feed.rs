use rikka_sync::{Condvar, Mutex};
use std::sync::Arc;
use std::time::Duration;

use rikka_gpu::{ExternalVideoFrame, VideoDeviceHandles};
use rikka_types::Size;

use crate::consume::FeedConsumer;
use crate::handoff::Handoff;

#[derive(Clone)]
pub struct Feed(Arc<Shared>);

struct Shared {
    handoff: Arc<Handoff<Arrival>>,
    device: Mutex<Option<FeedDevice>>,
    attached: Condvar,
}

#[derive(Debug)]
pub struct Arrival {
    pub width: u32,
    pub height: u32,
    pub content: Content,
}

#[derive(Debug)]
pub enum Content {
    Video(ExternalVideoFrame),
    Pixels(Vec<u8>),
}

#[derive(Clone, Debug)]
pub enum FeedDevice {
    Video(VideoDeviceHandles),
    NoVideo,
}

impl Default for Feed {
    fn default() -> Feed {
        Feed::new()
    }
}

impl Feed {
    #[must_use]
    pub fn new() -> Feed {
        Feed(Arc::new(Shared {
            handoff: Handoff::new(),
            device: Mutex::new(None),
            attached: Condvar::new(),
        }))
    }

    pub fn show(&self, width: u32, height: u32, pixels: Vec<u8>) {
        let expected = u64::from(width) * u64::from(height) * 4;
        if expected != pixels.len() as u64 {
            tracing::warn!(
                "{width}x{height} frame needs {expected} bytes, got {}",
                pixels.len()
            );
            return;
        }
        self.give(width, height, Content::Pixels(pixels));
    }

    pub fn show_video(&self, width: u32, height: u32, frame: ExternalVideoFrame) {
        self.give(width, height, Content::Video(frame));
    }

    pub fn wait_for_device(&self, patience: Duration) -> Option<FeedDevice> {
        let device = self.0.device.lock();
        let (device, _) = self
            .0
            .attached
            .wait_timeout_while(device, patience, |device| device.is_none());
        device.clone()
    }

    #[must_use]
    pub fn size(&self) -> Option<Size> {
        self.0.handoff.size()
    }

    #[must_use]
    pub fn id(&self) -> u64 {
        self.0.handoff.id()
    }

    fn give(&self, width: u32, height: u32, content: Content) {
        if width == 0 || height == 0 {
            tracing::warn!("a {width}x{height} frame cannot be shown");
            return;
        }
        self.0.handoff.give(
            width,
            height,
            Arrival {
                width,
                height,
                content,
            },
        );
    }
}

impl FeedConsumer for Feed {
    fn attach(&self, handles: Option<VideoDeviceHandles>) {
        let mut device = self.0.device.lock();
        if device.is_some() {
            return;
        }
        *device = Some(match handles {
            Some(handles) => FeedDevice::Video(handles),
            None => FeedDevice::NoVideo,
        });
        self.0.attached.notify_all();
    }

    fn is_attached(&self) -> bool {
        self.0.device.lock().is_some()
    }

    fn is_abandoned(&self) -> bool {
        Arc::strong_count(&self.0) == 1
    }

    fn take(&self) -> Option<Arrival> {
        self.0.handoff.take()
    }
}

impl std::fmt::Debug for Feed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Feed")
            .field("id", &self.id())
            .field("size", &self.size())
            .finish_non_exhaustive()
    }
}
