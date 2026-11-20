use std::time::Instant;

use ash::vk;

use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::image::{GpuImage, ImageAccess};
use rikka_gpu::core::runtime::{Recycler, UNUSED_FOR};
use rikka_gpu::core::table_image::TableImage;
use rikka_gpu::error::Result;

const SIZE_GRANULARITY: u32 = 64;
const CLASSES_PER_OCTAVE: u32 = 4;

pub(crate) const MAX_OVERSIZE: u32 = 4;

pub(crate) use rikka_gpu::core::convert::FX_FORMAT;

pub(crate) const HDR_FX_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;

pub(crate) fn size_class(texels: u32) -> u32 {
    let texels = texels.max(1);
    let step = (texels.next_power_of_two() / (2 * CLASSES_PER_OCTAVE)).max(SIZE_GRANULARITY);
    texels.next_multiple_of(step)
}

pub(crate) fn area(extent: vk::Extent2D) -> u32 {
    extent.width.saturating_mul(extent.height)
}

pub(crate) fn fits_without_waste(available: vk::Extent2D, required: vk::Extent2D) -> bool {
    available.width >= required.width
        && available.height >= required.height
        && area(available) <= area(required).saturating_mul(MAX_OVERSIZE)
}

pub(crate) fn fx_image(
    ctx: &mut GpuFrameCtx<'_>,
    name: &str,
    extent: vk::Extent2D,
    format: vk::Format,
) -> Result<TableImage> {
    let image = GpuImage::new(
        ctx.device.clone(),
        name,
        extent,
        format,
        vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
    )?;
    TableImage::register(image, ctx.resources, ctx.samplers.linear_clamp())
}

struct PoolTarget {
    table: TableImage,
    taken: u64,
    last_used: u64,
    used_at: Instant,
}

pub(crate) struct FxTargetPool {
    targets: Vec<PoolTarget>,
}

impl FxTargetPool {
    pub(crate) fn new() -> Self {
        Self {
            targets: Vec::new(),
        }
    }

    pub(crate) fn acquire(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        width: u32,
        height: u32,
        format: vk::Format,
    ) -> Result<usize> {
        let required = vk::Extent2D {
            width: size_class(width),
            height: size_class(height),
        };

        let candidate = self
            .targets
            .iter()
            .enumerate()
            .filter(|(_, target)| {
                target.taken != ctx.frame_value
                    && target.table.image.format() == format
                    && fits_without_waste(target.table.image.extent(), required)
            })
            .min_by_key(|(_, target)| area(target.table.image.extent()))
            .map(|(index, _)| index);

        let index = match candidate {
            Some(index) => index,
            None => {
                self.targets.push(PoolTarget {
                    table: fx_image(ctx, "rikka.fx", required, format)?,
                    taken: ctx.frame_value,
                    last_used: ctx.frame_value,
                    used_at: Instant::now(),
                });
                return Ok(self.targets.len() - 1);
            }
        };

        let target = &mut self.targets[index];
        target.taken = ctx.frame_value;
        target.last_used = ctx.frame_value;
        target.table.image.discard();
        Ok(index)
    }

    pub(crate) fn transition(
        &mut self,
        index: usize,
        next: ImageAccess,
    ) -> Option<vk::ImageMemoryBarrier2<'static>> {
        self.targets[index].table.image.transition(next)
    }

    pub(crate) fn image(&self, index: usize) -> &GpuImage {
        &self.targets[index].table.image
    }

    pub(crate) fn tex(&self, index: usize) -> u32 {
        self.targets[index].table.tex
    }

    pub(crate) fn release(&mut self, index: usize) {
        self.targets[index].taken = 0;
    }

    pub(crate) fn abandon(&mut self, frame_value: u64) {
        for target in &mut self.targets {
            if target.taken == frame_value {
                target.taken = 0;
            }
        }
    }

    pub(crate) fn end_frame(&mut self, ctx: &mut GpuFrameCtx<'_>, now: Instant) {
        for target in &mut self.targets {
            if target.last_used == ctx.frame_value {
                target.used_at = now;
            }
        }
        self.trim(&mut ctx.recycler(), now);
    }

    pub(crate) fn trim(&mut self, recycler: &mut Recycler<'_>, now: Instant) {
        let mut index = 0;
        while index < self.targets.len() {
            if now.duration_since(self.targets[index].used_at) >= UNUSED_FOR {
                let target = self.targets.swap_remove(index);
                target.table.retire(recycler);
            } else {
                index += 1;
            }
        }
    }
}
