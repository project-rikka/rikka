use std::sync::Arc;

use ash::vk;

use crate::renderer::Renderer;
use crate::renderer::copy_out::{record_copy, region};
use crate::target::OffscreenTarget;
use rikka_gpu::core::buffer::GpuBuffer;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::image::ImageAccess;
use rikka_gpu::core::memory::Placement;
#[cfg(unix)]
use rikka_gpu::drm::{SharedRows, export_rows};
use rikka_gpu::error::Result;
use rikka_gpu::reclaim::{Deferred, Reclaimer};
use rikka_gpu::texture::TexelRect;

impl Renderer {
    pub fn create_readback(&mut self) -> Readback {
        Readback {
            buffer: None,
            extent: (0, 0),
            ready_at: 0,
            claimed: 0,
            reclaimer: self.reclaimer.clone(),
        }
    }

    pub fn start_readback(
        &mut self,
        target: &mut OffscreenTarget,
        into: &mut Readback,
    ) -> Result<()> {
        let extent = target.image.extent();
        let whole = TexelRect::new([0, 0], [extent.width, extent.height]);
        self.start_readback_of(target, into, &[whole])
    }

    pub fn start_readback_of(
        &mut self,
        target: &mut OffscreenTarget,
        into: &mut Readback,
        areas: &[TexelRect],
    ) -> Result<()> {
        let extent = target.image.extent();
        let regions: Vec<vk::BufferImageCopy> = areas
            .iter()
            .filter_map(|area| region(*area, extent, extent.width))
            .collect();
        if regions.is_empty() {
            return Ok(());
        }
        let buffer = into.reserve(self.gpu.device(), self.gpu.completed_value(), extent)?;
        let device = self.gpu.device().clone();
        let ready_at = self.gpu.record_once(|cmd| {
            target.image.barrier_to(cmd, ImageAccess::TRANSFER_SRC);
            record_copy(&device, cmd, target.image.handle(), buffer, &regions);
        })?;
        into.arm(ready_at, (extent.width, extent.height));
        Ok(())
    }

    #[cfg(unix)]
    pub fn export_rows(&mut self, width: u32, height: u32) -> Result<SharedRows> {
        let (memory, fd, stride) = export_rows(self.gpu.device().clone(), width, height)?;
        Ok(SharedRows {
            fd,
            width,
            height,
            stride,
            memory: self.reclaimer.defer(memory),
        })
    }

    pub fn poll_readback(&mut self, from: &mut Readback) -> bool {
        if !from.in_flight() || self.gpu.completed_value() < from.ready_at {
            return false;
        }
        from.claim()
    }

    pub fn wait_for_pixels(&mut self, from: &mut Readback) -> Result<bool> {
        if !from.in_flight() {
            return Ok(false);
        }
        self.gpu.wait_value(from.ready_at)?;
        Ok(self.poll_readback(from))
    }

    pub fn read_pixels(&mut self, target: &mut OffscreenTarget) -> Result<Vec<u8>> {
        let mut readback = self.create_readback();
        self.start_readback(target, &mut readback)?;
        self.wait_for_pixels(&mut readback)?;
        Ok(readback.pixels().to_vec())
    }
}

pub struct Readback {
    buffer: Option<Deferred<GpuBuffer>>,
    extent: (u32, u32),
    ready_at: u64,
    claimed: usize,
    reclaimer: Reclaimer,
}

impl Readback {
    #[must_use]
    pub fn in_flight(&self) -> bool {
        self.ready_at != 0
    }

    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        self.buffer
            .as_ref()
            .and_then(|buffer| buffer.mapped_slice())
            .map_or(&[], |mapped| &mapped[..self.claimed])
    }

    #[must_use]
    pub fn extent(&self) -> (u32, u32) {
        self.extent
    }

    pub(super) fn reserve(
        &mut self,
        device: &Arc<RenderDevice>,
        completed: u64,
        extent: vk::Extent2D,
    ) -> Result<vk::Buffer> {
        let size = u64::from(extent.width) * u64::from(extent.height) * 4;
        if self.ready_at > completed
            || self
                .buffer
                .as_ref()
                .is_none_or(|buffer| buffer.size() < size)
        {
            self.buffer = Some(self.reclaimer.defer(GpuBuffer::new(
                device.clone(),
                "rikka.readback",
                size,
                vk::BufferUsageFlags::TRANSFER_DST,
                Placement::Readback,
            )?));
        }
        Ok(self.buffer.as_ref().expect("buffer exists").handle())
    }

    pub(super) fn arm(&mut self, ready_at: u64, extent: (u32, u32)) {
        self.ready_at = ready_at;
        self.extent = extent;
        self.claimed = 0;
    }

    fn claim(&mut self) -> bool {
        self.ready_at = 0;
        let filled = self.extent.0 as usize * self.extent.1 as usize * 4;
        let available = self
            .buffer
            .as_ref()
            .is_some_and(|buffer| buffer.size() >= filled as u64);
        self.claimed = if available { filled } else { 0 };
        available
    }
}

impl std::fmt::Debug for Readback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Readback")
            .field("size", &self.extent)
            .field("in_flight", &self.in_flight())
            .finish_non_exhaustive()
    }
}
