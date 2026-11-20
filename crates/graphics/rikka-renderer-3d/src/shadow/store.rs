use std::sync::Arc;

use ash::vk;

use crate::error::Result;
use crate::shadow::plan::Planned;
use crate::shadow::{Atlas, AtlasAllocator, ShadowCache};
use crate::viewport::DEPTH_FORMAT;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::image::{GpuImage, ImageDesc};

pub(crate) const TINT_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;

pub(crate) struct Sampled {
    pub image: GpuImage,
    pub slot: u32,
}

pub(crate) struct Tint {
    pub color: Sampled,
    pub depth: Sampled,
}

pub(crate) struct FigureAtlas {
    pub image: Sampled,
    pub side: u32,
    pub allocator: AtlasAllocator,
}

pub(crate) struct ShadowAtlas {
    pub image: GpuImage,
    pub slot: u32,
    pub side: u32,
    pub generation: u64,
    pub allocator: AtlasAllocator,
    pub cache: ShadowCache,
    pub tint: Option<Tint>,
    pub figures: Option<FigureAtlas>,
}

impl ShadowAtlas {
    pub(crate) fn tint_side(&self) -> u32 {
        self.side / 2
    }

    pub(crate) fn side_of(&self, atlas: Atlas) -> u32 {
        match atlas {
            Atlas::Main => self.side,
            Atlas::Figures => self.figures.as_ref().map_or(1, |figures| figures.side),
        }
    }
}

fn square(side: u32) -> vk::Extent2D {
    vk::Extent2D {
        width: side,
        height: side,
    }
}

fn depth_image(
    device: &Arc<RenderDevice>,
    ctx: &mut GpuFrameCtx<'_>,
    name: &str,
    side: u32,
) -> Result<Sampled> {
    let image = GpuImage::create(
        device.clone(),
        name,
        &ImageDesc::depth(
            square(side),
            DEPTH_FORMAT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        ),
    )?;
    let slot = ctx
        .resources
        .allocate(image.view(), ctx.samplers.shadow())?;
    Ok(Sampled { image, slot })
}

fn retire(ctx: &mut GpuFrameCtx<'_>, sampled: Sampled) {
    ctx.resources.release(sampled.slot, ctx.frame_value);
    ctx.retire.defer(ctx.frame_value, Box::new(sampled.image));
}

fn tint(device: &Arc<RenderDevice>, ctx: &mut GpuFrameCtx<'_>, side: u32) -> Result<Tint> {
    let image = GpuImage::create(
        device.clone(),
        "rikka.three.shadow_tint",
        &ImageDesc::color(
            square(side),
            TINT_FORMAT,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        ),
    )?;
    let slot = ctx
        .resources
        .allocate(image.view(), ctx.samplers.linear_clamp())?;
    Ok(Tint {
        color: Sampled { image, slot },
        depth: depth_image(device, ctx, "rikka.three.shadow_tint_depth", side)?,
    })
}

pub(crate) fn ensure(
    atlas: &mut Option<ShadowAtlas>,
    device: &Arc<RenderDevice>,
    ctx: &mut GpuFrameCtx<'_>,
    planned: Planned,
    tinted: bool,
) -> Result<()> {
    let side = planned.side.max(1);
    let current = match atlas {
        Some(current) if current.side >= side => {
            if tinted && current.tint.is_none() {
                current.tint = Some(tint(device, ctx, current.tint_side())?);
                current.cache.forget();
            }
            current
        }
        _ => {
            let generation = atlas.as_ref().map_or(1, |atlas| atlas.generation + 1);
            let sampled = depth_image(device, ctx, "rikka.three.shadow_atlas", side)?;
            let tint = match tinted {
                true => Some(tint(device, ctx, side / 2)?),
                false => None,
            };
            let mut figures = None;
            if let Some(outgrown) = atlas.take() {
                figures = outgrown.figures;
                retire(
                    ctx,
                    Sampled {
                        image: outgrown.image,
                        slot: outgrown.slot,
                    },
                );
                if let Some(tint) = outgrown.tint {
                    retire(ctx, tint.color);
                    retire(ctx, tint.depth);
                }
            }
            atlas.insert(ShadowAtlas {
                image: sampled.image,
                slot: sampled.slot,
                side,
                generation,
                allocator: AtlasAllocator::new(side),
                cache: ShadowCache::default(),
                tint,
                figures,
            })
        }
    };
    let fits = |figures: &FigureAtlas| figures.side >= planned.figures;
    if planned.figures == 0 || current.figures.as_ref().is_some_and(fits) {
        return Ok(());
    }
    let sampled = depth_image(device, ctx, "rikka.three.figure_atlas", planned.figures)?;
    if let Some(outgrown) = current.figures.replace(FigureAtlas {
        image: sampled,
        side: planned.figures,
        allocator: AtlasAllocator::new(planned.figures),
    }) {
        retire(ctx, outgrown.image);
    }
    Ok(())
}
