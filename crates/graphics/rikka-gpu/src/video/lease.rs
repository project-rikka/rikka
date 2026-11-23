use std::any::Any;
use std::sync::Arc;

use ash::vk;

use crate::video::frame::VideoFrameImage;
use crate::video::releaser::VideoReleaser;

pub trait LentFrame: Send + 'static {
    fn claim(&mut self) -> FrameClaim;
}

#[derive(Debug)]
pub struct FrameClaim {
    pub images: Vec<VideoFrameImage>,
    pub waits: Vec<(vk::Semaphore, u64)>,
    pub signals: Vec<(vk::Semaphore, u64)>,
}

pub struct VideoLease {
    releaser: Arc<VideoReleaser>,
    frame: Option<Box<dyn LentFrame>>,
    claim: Option<FrameClaim>,
}

impl VideoLease {
    pub(crate) fn new(releaser: Arc<VideoReleaser>, frame: Box<dyn LentFrame>) -> Self {
        Self {
            releaser,
            frame: Some(frame),
            claim: None,
        }
    }

    pub(crate) fn claim(&mut self) -> &FrameClaim {
        let frame = self
            .frame
            .as_mut()
            .expect("a lease holds its frame until it settles");
        self.claim.get_or_insert_with(|| frame.claim())
    }

    pub(crate) fn claimed(&self) -> Option<&FrameClaim> {
        self.claim.as_ref()
    }

    pub(crate) fn unclaim(&mut self) {
        if let Some(claim) = self.claim.take() {
            self.releaser.release(&claim);
        }
    }

    pub(crate) fn settle(mut self) -> Box<dyn Any + Send> {
        self.claim = None;
        Box::new(
            self.frame
                .take()
                .expect("a lease holds its frame until it settles"),
        )
    }
}

impl Drop for VideoLease {
    fn drop(&mut self) {
        self.unclaim();
    }
}

impl std::fmt::Debug for VideoLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoLease")
            .field("claim", &self.claim)
            .finish_non_exhaustive()
    }
}
