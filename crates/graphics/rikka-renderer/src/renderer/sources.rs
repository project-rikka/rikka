use std::sync::Arc;

use ash::vk;

use rikka_gpu::core::convert::YcbcrConverter;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::runtime::{GpuRuntime, Recycler};
use rikka_gpu::core::transfer::TransferLane;
use rikka_gpu::error::Result;
use rikka_gpu::texture::{TextureBook, TextureId, TexturePool};
use rikka_gpu::video::{VideoImports, VideoSemaphores};

pub(crate) struct Sources {
    pub textures: TexturePool,
    pub video: VideoImports,
    pub convert: YcbcrConverter,
    pub transfer: Option<TransferLane>,
}

impl Sources {
    pub(crate) fn new(
        device: Arc<RenderDevice>,
        table_layout: vk::DescriptorSetLayout,
        book: Arc<TextureBook>,
        transfer: Option<TransferLane>,
    ) -> Self {
        Self {
            textures: TexturePool::new(book, transfer.as_ref().map(TransferLane::deferred)),
            video: VideoImports::new(device.clone()),
            convert: YcbcrConverter::new(device, table_layout),
            transfer,
        }
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.textures.has_pending() || self.video.has_pending()
    }

    pub(crate) fn stream(&mut self, gpu: &GpuRuntime) {
        let Some(lane) = self.transfer.as_mut() else {
            return;
        };
        let last_frame = gpu.next_value() - 1;
        if let Err(error) = self.textures.stream(lane, gpu.timeline(), last_frame) {
            tracing::warn!("failed to submit uploads, deferring to next frame: {error}");
        }
    }

    pub(crate) fn trim(&mut self) {
        if let Some(lane) = self.transfer.as_mut() {
            lane.collect();
        }
    }

    pub(crate) fn record(&mut self, ctx: &mut GpuFrameCtx<'_>) -> Result<VideoSemaphores> {
        let computed = self.textures.flush(ctx, &mut self.convert)?;
        let mut semaphores = self
            .video
            .record(ctx, &mut self.textures, &mut self.convert)?;
        semaphores.waits.extend(computed);
        Ok(semaphores)
    }

    pub(crate) fn commit(&mut self, gpu: &mut GpuRuntime, submitted: u64) {
        let computed = self.textures.sent_computed();
        if !computed.is_empty() {
            gpu.defer_until(submitted, Box::new(computed));
        }
        for former in self.textures.commit() {
            gpu.defer_until(submitted, Box::new(former));
        }
        for keepalive in self.video.commit() {
            gpu.defer_until(submitted, keepalive);
        }
    }

    pub(crate) fn revert(&mut self) {
        self.textures.revert();
        self.video.revert();
    }

    pub(crate) fn destroy_texture(&mut self, recycler: &mut Recycler<'_>, id: TextureId) -> bool {
        if !self.textures.destroy(recycler, id) {
            return false;
        }
        self.video.forget_texture(id);
        true
    }
}
