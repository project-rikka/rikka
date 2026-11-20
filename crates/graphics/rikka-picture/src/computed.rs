use rikka_sync::{Mutex, MutexGuard};
use std::sync::Arc;

use rikka_gpu::ComputedFrame;
use rikka_types::Size;

use crate::consume::ComputedConsumer;
use crate::handoff::{Handoff, announce_arrival};

type Notify = Box<dyn FnOnce() + Send>;

#[derive(Clone)]
pub struct ComputedImage {
    frames: Arc<Handoff<ComputedFrame>>,
    uploaded: Arc<Mutex<Vec<Notify>>>,
}

impl Default for ComputedImage {
    fn default() -> ComputedImage {
        ComputedImage::new()
    }
}

impl ComputedImage {
    #[must_use]
    pub fn new() -> ComputedImage {
        ComputedImage {
            frames: Handoff::new(),
            uploaded: Arc::default(),
        }
    }

    pub fn show(&self, frame: ComputedFrame) {
        let (width, height) = (frame.width(), frame.height());
        if !frame.is_ready() {
            frame.on_ready(announce_arrival);
        }
        self.frames.give(width, height, frame);
    }

    #[must_use]
    pub fn size(&self) -> Option<Size> {
        self.frames.size()
    }

    pub fn on_uploaded(&self, notify: impl FnOnce() + Send + 'static) {
        let mut uploaded = self.uploaded();
        if self.frames.is_waiting() {
            uploaded.push(Box::new(notify));
            return;
        }
        drop(uploaded);
        notify();
    }

    #[must_use]
    pub fn id(&self) -> u64 {
        self.frames.id()
    }

    fn upload_with(
        &self,
        take: impl FnOnce(&Handoff<ComputedFrame>) -> Option<ComputedFrame>,
        upload: impl FnOnce(ComputedFrame),
    ) {
        let mut uploaded = self.uploaded();
        let Some(frame) = take(&self.frames) else {
            return;
        };
        upload(frame);
        let notify = std::mem::take(&mut *uploaded);
        drop(uploaded);
        for notify in notify {
            notify();
        }
    }

    fn uploaded(&self) -> MutexGuard<'_, Vec<Notify>> {
        self.uploaded.lock()
    }
}

impl ComputedConsumer for ComputedImage {
    fn is_abandoned(&self) -> bool {
        Arc::strong_count(&self.frames) == 1
    }

    fn upload_now(&self, upload: impl FnOnce(ComputedFrame)) {
        self.upload_with(Handoff::take, upload);
    }

    fn upload_when_ready(&self, upload: impl FnOnce(ComputedFrame)) {
        self.upload_with(|frames| frames.take_if(ComputedFrame::is_ready), upload);
    }
}

impl std::fmt::Debug for ComputedImage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ComputedImage")
            .field("id", &self.frames.id())
            .field("size", &self.size())
            .finish_non_exhaustive()
    }
}
