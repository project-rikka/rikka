#[cfg(unix)]
use std::os::fd::OwnedFd;

use ash::vk;
use rikka_colorimetry::BlendSpace;
use rikka_types::{Rect, Transform};

use crate::renderer::copy_out::CopyOut;
use crate::renderer::draw::RenderOutput;
use crate::renderer::history::DamageSource;
use crate::scene::{Scene, Viewer};
use crate::transform::turned_extent;
use rikka_gpu::core::image::{ImageAccess, QueueTransfer};
use rikka_gpu::core::runtime::GpuRuntime;
use rikka_gpu::error::Result;
use rikka_gpu::identity::{BufferId, TargetId};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Plan {
    pub(crate) target: TargetId,
    pub(crate) buffer: BufferId,
    pub(crate) extent: vk::Extent2D,
    pub(crate) transform: Transform,
    pub(crate) damage: DamageSource,
    pub(crate) readable: bool,
    pub(crate) viewer: Viewer,
    pub(crate) space: BlendSpace,
}

impl Plan {
    pub(crate) fn new(target: TargetId, extent: vk::Extent2D) -> Self {
        Self {
            target,
            buffer: BufferId::Sole,
            extent,
            transform: Transform::Normal,
            damage: DamageSource::SinceLastFrame,
            readable: false,
            viewer: Viewer::Screen,
            space: BlendSpace::Sdr,
        }
    }

    pub(crate) fn blended_in(mut self, space: BlendSpace) -> Self {
        self.space = space;
        self
    }

    pub(crate) fn viewed_by(mut self, viewer: Viewer) -> Self {
        self.viewer = viewer;
        self
    }

    pub(crate) fn onto_buffer(mut self, buffer: BufferId) -> Self {
        self.buffer = buffer;
        self
    }

    pub(crate) fn transformed(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    pub(crate) fn damaged(mut self, damage: DamageSource) -> Self {
        self.damage = damage;
        self
    }

    pub(crate) fn read_back(mut self, readable: bool) -> Self {
        self.readable = readable;
        self
    }

    pub(crate) fn scene_extent(&self) -> vk::Extent2D {
        let (width, height) =
            turned_extent(self.transform, (self.extent.width, self.extent.height));
        vk::Extent2D { width, height }
    }

    pub(crate) fn viewport(&self) -> Rect {
        let extent = self.scene_extent();
        Rect::new(0.0, 0.0, extent.width as f32, extent.height as f32)
    }

    pub(crate) fn composes(&self, scene: &Scene) -> bool {
        self.space.is_hdr()
            || !scene.effects().is_empty()
            || (scene.is_immediate() && self.damage == DamageSource::SinceLastFrame)
            || (!self.readable && (scene.has_backdrops() || scene.has_fades()))
    }
}

#[derive(Debug)]
pub struct Canvas {
    pub(crate) plan: Plan,
    pub(crate) image: vk::Image,
    pub(crate) view: vk::ImageView,
    pub(crate) format: vk::Format,
    pub(crate) texels: vk::Extent2D,
    pub(crate) before: ImageAccess,
    pub(crate) after: ImageAccess,
    pub(crate) acquire: QueueTransfer,
    pub(crate) release: QueueTransfer,
    pub(crate) wait: Option<vk::SemaphoreSubmitInfo<'static>>,
    pub(crate) signal: Option<vk::SemaphoreSubmitInfo<'static>>,
    pub(crate) underlay: bool,
}

impl Canvas {
    pub(crate) fn new(
        plan: Plan,
        image: vk::Image,
        view: vk::ImageView,
        format: vk::Format,
    ) -> Self {
        Self {
            texels: plan.extent,
            plan,
            image,
            view,
            format,
            before: ImageAccess::UNDEFINED,
            after: ImageAccess::COLOR_ATTACHMENT,
            acquire: QueueTransfer::NONE,
            release: QueueTransfer::NONE,
            wait: None,
            signal: None,
            underlay: false,
        }
    }

    pub(crate) fn over_contents(mut self) -> Self {
        self.underlay = true;
        self
    }

    #[cfg(unix)]
    pub(crate) fn with_extent(mut self, texels: vk::Extent2D) -> Self {
        self.texels = texels;
        self
    }

    pub(crate) fn with_layouts(mut self, before: ImageAccess, after: ImageAccess) -> Self {
        self.before = before;
        self.after = after;
        self
    }

    #[cfg(unix)]
    pub(crate) fn transferred(mut self, acquire: QueueTransfer, release: QueueTransfer) -> Self {
        self.acquire = acquire;
        self.release = release;
        self
    }

    pub(crate) fn waits_on(
        mut self,
        semaphore: vk::Semaphore,
        stage: vk::PipelineStageFlags2,
    ) -> Self {
        self.wait = Some(submit_info(semaphore, 0, stage));
        self
    }

    pub(crate) fn signals(
        mut self,
        semaphore: vk::Semaphore,
        value: u64,
        stage: vk::PipelineStageFlags2,
    ) -> Self {
        self.signal = Some(submit_info(semaphore, value, stage));
        self
    }

    pub(crate) fn entry(&self) -> ImageAccess {
        let waited = self
            .wait
            .map_or(vk::PipelineStageFlags2::NONE, |wait| wait.stage_mask);
        self.before.after_waiting(waited)
    }
}

fn submit_info(
    semaphore: vk::Semaphore,
    value: u64,
    stage: vk::PipelineStageFlags2,
) -> vk::SemaphoreSubmitInfo<'static> {
    vk::SemaphoreSubmitInfo::default()
        .semaphore(semaphore)
        .value(value)
        .stage_mask(stage)
}

#[derive(Debug)]
pub struct Painted<'a> {
    pub(crate) output: RenderOutput,
    pub(crate) submitted: u64,
    #[cfg_attr(
        not(unix),
        expect(dead_code, reason = "only dmabuf fences read the runtime")
    )]
    pub(crate) gpu: &'a mut GpuRuntime,
}

#[cfg(unix)]
impl Painted<'_> {
    pub(crate) fn readable_when_done(&mut self) -> Result<OwnedFd> {
        self.gpu.readable_after(self.submitted)
    }
}

pub trait Onto {
    type Output;

    fn canvas(&mut self) -> Canvas;

    #[cfg(unix)]
    fn released_by(&mut self) -> Option<OwnedFd> {
        None
    }

    fn copies_out(&mut self) -> Result<Option<CopyOut<'_>>> {
        Ok(None)
    }

    fn has_copy_work(&self) -> bool {
        false
    }

    fn undrawn(&mut self, output: RenderOutput) -> Self::Output;

    fn drawn(&mut self, painted: Painted<'_>) -> Result<Self::Output>;
}
