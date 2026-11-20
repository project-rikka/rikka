use std::sync::Arc;
use std::time::Duration;

use crate::renderer::Renderer;
use rikka_gpu::Gpu;
use rikka_gpu::core::frame::FrameTimeline;

impl Renderer {
    #[must_use]
    pub fn gpu(&self) -> &Gpu {
        &self.device
    }

    pub fn timeline(&self) -> Arc<FrameTimeline> {
        Arc::clone(self.gpu.timeline())
    }

    pub fn submitted_frame(&self) -> u64 {
        self.gpu.next_value().saturating_sub(1)
    }

    pub fn has_free_frame(&self) -> bool {
        self.gpu.has_free_frame_slot()
    }

    pub fn next_free_frame(&self) -> Option<u64> {
        self.gpu.next_free_frame()
    }

    pub fn take_stall_time(&mut self) -> Duration {
        self.gpu.take_stall()
    }
}
