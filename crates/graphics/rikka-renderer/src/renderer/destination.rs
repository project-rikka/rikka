use ash::vk;
use rikka_colorimetry::BlendSpace;
use rikka_types::Transform;

use crate::renderer::Renderer;
use crate::renderer::canvas::{Canvas, Onto, Painted, Plan};
use crate::renderer::draw::RenderOutput;
use crate::renderer::history::DamageSource;
use crate::scene::Viewer;
use crate::target::{OffscreenTarget, TargetFormat};
use rikka_gpu::core::image::{GpuImage, ImageAccess};
use rikka_gpu::error::Result;
use rikka_gpu::reclaim::TargetLease;

impl Renderer {
    pub fn create_target(&mut self, width: u32, height: u32) -> Result<OffscreenTarget> {
        self.create_target_as(width, height, TargetFormat::Rgba8)
    }

    pub fn create_target_as(
        &mut self,
        width: u32,
        height: u32,
        format: TargetFormat,
    ) -> Result<OffscreenTarget> {
        let image = GpuImage::new(
            self.gpu.device().clone(),
            "rikka.offscreen",
            vk::Extent2D { width, height },
            format.vk_format(),
            vk::ImageUsageFlags::COLOR_ATTACHMENT
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::TRANSFER_SRC,
        )?;
        Ok(OffscreenTarget {
            image: self.reclaimer.defer(image),
            lease: self.lease(),
            transform: Transform::Normal,
            viewer: Viewer::Screen,
            space: BlendSpace::Sdr,
        })
    }

    pub(crate) fn lease(&mut self) -> TargetLease {
        self.reclaimer.lease(self.targets.new_target())
    }
}

impl Onto for &mut OffscreenTarget {
    type Output = RenderOutput;

    fn canvas(&mut self) -> Canvas {
        let image = &*self.image;
        let plan = Plan::new(self.lease.id, image.extent())
            .transformed(self.transform)
            .damaged(DamageSource::CompleteForBuffer)
            .read_back(true)
            .viewed_by(self.viewer)
            .blended_in(self.space);
        Canvas::new(plan, image.handle(), image.view(), image.format())
            .with_layouts(image.access(), ImageAccess::COLOR_ATTACHMENT)
    }

    fn undrawn(&mut self, output: RenderOutput) -> RenderOutput {
        output
    }

    fn drawn(&mut self, painted: Painted<'_>) -> Result<RenderOutput> {
        self.image.set_access(ImageAccess::COLOR_ATTACHMENT);
        Ok(painted.output)
    }
}
