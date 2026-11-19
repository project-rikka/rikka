use ash::vk;
use rikka_types::Transform;

use crate::renderer::Renderer;
use crate::renderer::canvas::{Canvas, Onto, Painted, Plan};
use crate::renderer::draw::RenderOutput;
use crate::renderer::history::DamageSource;
use crate::scene::Viewer;
use rikka_gpu::core::image::ImageAccess;
use rikka_gpu::error::Result;
use rikka_gpu::reclaim::TargetLease;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExternalImage {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub format: vk::Format,
    pub width: u32,
    pub height: u32,
    pub layout: vk::ImageLayout,
    pub transform: Transform,
}

#[derive(Debug)]
pub struct ExternalTarget {
    lease: TargetLease,
    viewer: Viewer,
}

impl ExternalTarget {
    pub fn set_viewer(&mut self, viewer: Viewer) {
        self.viewer = viewer;
    }

    pub unsafe fn onto(&mut self, image: ExternalImage) -> ExternalDraw<'_> {
        ExternalDraw {
            target: self,
            image,
        }
    }
}

#[derive(Debug)]
#[must_use = "an external image is drawn onto by handing this to draw()"]
pub struct ExternalDraw<'a> {
    target: &'a mut ExternalTarget,
    image: ExternalImage,
}

impl Renderer {
    pub fn create_external_target(&mut self) -> ExternalTarget {
        ExternalTarget {
            lease: self.lease(),
            viewer: Viewer::Screen,
        }
    }
}

impl Onto for ExternalDraw<'_> {
    type Output = RenderOutput;

    fn canvas(&mut self) -> Canvas {
        let image = self.image;
        let extent = vk::Extent2D {
            width: image.width,
            height: image.height,
        };
        let plan = Plan::new(self.target.lease.id, extent)
            .transformed(image.transform)
            .damaged(DamageSource::CompleteForBuffer)
            .read_back(true)
            .viewed_by(self.target.viewer);
        let written = ImageAccess {
            stage: vk::PipelineStageFlags2::ALL_COMMANDS,
            access: vk::AccessFlags2::MEMORY_WRITE,
            layout: image.layout,
        };
        let handed_back = ImageAccess {
            access: vk::AccessFlags2::MEMORY_READ | vk::AccessFlags2::MEMORY_WRITE,
            ..written
        };
        Canvas::new(plan, image.image, image.view, image.format)
            .with_layouts(written, handed_back)
            .over_contents()
    }

    fn undrawn(&mut self, output: RenderOutput) -> RenderOutput {
        output
    }

    fn drawn(&mut self, painted: Painted<'_>) -> Result<RenderOutput> {
        Ok(painted.output)
    }
}
