use ash::vk;

use crate::renderer::Renderer;
use crate::renderer::canvas::{Canvas, Onto, Painted, Plan};
use crate::renderer::draw::{FrameStart, RenderOutput};
use crate::renderer::history::DamageSource;
use crate::scene::Scene;
use rikka_gpu::core::image::ImageAccess;
use rikka_gpu::error::Result;
use rikka_gpu::identity::BufferId;
use rikka_gpu::reclaim::{ReclaimRequest, Reclaimer, TargetLease};
use rikka_gpu::{SurfaceFit, Swapchain};

const SUBMISSION: vk::PipelineStageFlags2 = vk::PipelineStageFlags2::ALL_COMMANDS;

#[derive(Debug)]
pub struct SwapchainOutput {
    lease: TargetLease,
    swapchain: Option<u64>,
    buffers: Vec<Option<BufferId>>,
    reclaimer: Reclaimer,
}

impl SwapchainOutput {
    fn plan(fit: SurfaceFit, lease: &TargetLease) -> Plan {
        Plan::new(lease.id, fit.extent)
            .damaged(DamageSource::SinceLastFrame)
            .read_back(fit.readable)
    }

    fn forget_buffers(&mut self) {
        for buffer in self.buffers.drain(..).flatten() {
            self.reclaimer.enqueue(ReclaimRequest::Buffer(buffer));
        }
    }
}

impl Drop for SwapchainOutput {
    fn drop(&mut self) {
        self.forget_buffers();
    }
}

impl Renderer {
    pub fn create_swapchain_output(&mut self) -> SwapchainOutput {
        SwapchainOutput {
            lease: self.lease(),
            swapchain: None,
            buffers: Vec::new(),
            reclaimer: self.reclaimer.clone(),
        }
    }

    pub fn start_swapchain(
        &mut self,
        scene: &mut Scene,
        output: &SwapchainOutput,
        fit: SurfaceFit,
    ) -> Result<FrameStart> {
        self.start_plan(scene, SwapchainOutput::plan(fit, &output.lease))
    }

    pub fn swapchain_draw<'a>(
        &mut self,
        output: &'a mut SwapchainOutput,
        swapchain: &'a mut Swapchain,
        index: u32,
    ) -> SwapchainDraw<'a> {
        if output.swapchain != Some(swapchain.id()) {
            output.forget_buffers();
            output.buffers = vec![None; swapchain.image_count()];
            output.swapchain = Some(swapchain.id());
        }
        let buffer =
            *output.buffers[index as usize].get_or_insert_with(|| self.targets.new_external());
        SwapchainDraw {
            plan: SwapchainOutput::plan(
                SurfaceFit {
                    extent: swapchain.extent(),
                    readable: swapchain.is_readable(),
                },
                &output.lease,
            )
            .onto_buffer(buffer),
            swapchain,
            index,
        }
    }
}

#[derive(Debug)]
#[must_use = "a swapchain image is drawn onto by handing this to draw()"]
pub struct SwapchainDraw<'a> {
    plan: Plan,
    swapchain: &'a mut Swapchain,
    index: u32,
}

#[derive(Debug)]
pub enum SwapchainFrame {
    Untouched(RenderOutput),
    Submitted(RenderOutput),
}

impl Onto for SwapchainDraw<'_> {
    type Output = SwapchainFrame;

    fn canvas(&mut self) -> Canvas {
        let format = self.swapchain.format();
        let image = self.swapchain.image(self.index);
        Canvas::new(self.plan, image.handle(), image.view(), format)
            .with_layouts(image.access(), ImageAccess::PRESENT)
            .waits_on(image.acquired(), SUBMISSION)
            .signals(image.rendered(), 0, SUBMISSION)
    }

    fn undrawn(&mut self, output: RenderOutput) -> SwapchainFrame {
        SwapchainFrame::Untouched(output)
    }

    fn drawn(&mut self, painted: Painted<'_>) -> Result<SwapchainFrame> {
        self.swapchain
            .image_mut(self.index)
            .set_access(ImageAccess::PRESENT);
        Ok(SwapchainFrame::Submitted(painted.output))
    }
}
