use std::any::Any;
use std::sync::Arc;

use ash::vk;

use crate::video::frame::VideoFrameImage;
use crate::video::releaser::VideoReleaser;

pub struct LentFrame {
    pub images: Vec<VideoFrameImage>,
    pub waits: Vec<(vk::Semaphore, u64)>,
    pub signals: Vec<(vk::Semaphore, u64)>,
    pub keepalive: Box<dyn Any + Send>,
}

pub struct VideoLease {
    releaser: Arc<VideoReleaser>,
    lent: Option<LentFrame>,
}

impl VideoLease {
    pub(crate) fn new(releaser: Arc<VideoReleaser>, lent: LentFrame) -> Self {
        Self {
            releaser,
            lent: Some(lent),
        }
    }

    pub(crate) fn lent(&self) -> &LentFrame {
        self.lent
            .as_ref()
            .expect("a lease holds its frame until it settles")
    }

    pub(crate) fn settle(mut self) -> Box<dyn Any + Send> {
        self.lent
            .take()
            .expect("a lease holds its frame until it settles")
            .keepalive
    }
}

impl Drop for VideoLease {
    fn drop(&mut self) {
        if let Some(lent) = self.lent.take() {
            self.releaser.release(lent);
        }
    }
}

impl std::fmt::Debug for LentFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LentFrame")
            .field("images", &self.images)
            .field("waits", &self.waits)
            .field("signals", &self.signals)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for VideoLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoLease")
            .field("lent", &self.lent)
            .finish_non_exhaustive()
    }
}
