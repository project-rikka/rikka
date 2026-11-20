use std::os::fd::OwnedFd;

use ash::vk;
use rikka_colorimetry::BlendSpace;
use rikka_types::Transform;

use crate::renderer::Renderer;
use crate::renderer::canvas::{Canvas, Onto, Painted, Plan};
use crate::renderer::draw::{FrameStart, RenderOutput};
use crate::renderer::history::DamageSource;
use crate::scene::{Scene, Viewer};
use rikka_gpu::core::image::{ImageAccess, QueueTransfer};
use rikka_gpu::drm::{
    DmabufSpec, DmabufTarget, ExportSemaphore, ExportedDmabuf, Placement, export_render,
    import_render,
};
use rikka_gpu::error::Result;
use rikka_gpu::reclaim::TargetLease;

#[derive(Debug)]
pub struct DmabufOutput {
    lease: TargetLease,
    transform: Transform,
    damage: DamageSource,
    readable: bool,
    viewer: Viewer,
    space: BlendSpace,
}

impl DmabufOutput {
    pub fn set_transform(&mut self, transform: Transform) {
        self.transform = transform;
    }

    pub fn transform(&self) -> Transform {
        self.transform
    }

    pub fn set_viewer(&mut self, viewer: Viewer) {
        self.viewer = viewer;
    }

    pub fn set_blend_space(&mut self, space: BlendSpace) {
        self.space = space;
    }

    pub fn blend_space(&self) -> BlendSpace {
        self.space
    }

    pub(crate) fn plan(&self, extent: vk::Extent2D) -> Plan {
        Plan::new(self.lease.id, extent)
            .transformed(self.transform)
            .damaged(self.damage)
            .read_back(self.readable)
            .viewed_by(self.viewer)
            .blended_in(self.space)
    }
}

#[derive(Debug)]
pub enum DmabufFence {
    SyncFile(OwnedFd),
    Readable(OwnedFd),
}

impl DmabufFence {
    pub fn into_readable(self) -> OwnedFd {
        match self {
            Self::SyncFile(fd) | Self::Readable(fd) => fd,
        }
    }
}

#[derive(Debug)]
pub struct DmabufFrame {
    pub output: RenderOutput,
    pub fence: Option<DmabufFence>,
}

impl Renderer {
    pub fn create_dmabuf_output(&mut self, damage: DamageSource) -> DmabufOutput {
        DmabufOutput {
            lease: self.lease(),
            transform: Transform::Normal,
            damage,
            readable: true,
            viewer: Viewer::Screen,
            space: BlendSpace::Sdr,
        }
    }

    pub fn start_dmabuf(
        &mut self,
        scene: &mut Scene,
        output: &DmabufOutput,
        width: u32,
        height: u32,
    ) -> Result<FrameStart> {
        self.start_plan(scene, output.plan(vk::Extent2D { width, height }))
    }

    pub fn export_dmabuf(
        &mut self,
        width: u32,
        height: u32,
        fourcc: u32,
        modifiers: &[u64],
    ) -> Result<ExportedDmabuf> {
        self.export(width, height, fourcc, modifiers, Placement::Device)
    }

    pub fn export_shared_dmabuf(
        &mut self,
        width: u32,
        height: u32,
        fourcc: u32,
        modifiers: &[u64],
    ) -> Result<ExportedDmabuf> {
        self.export(width, height, fourcc, modifiers, Placement::System)
    }

    fn export(
        &mut self,
        width: u32,
        height: u32,
        fourcc: u32,
        modifiers: &[u64],
        placement: Placement,
    ) -> Result<ExportedDmabuf> {
        let device = self.gpu.device().clone();
        let exported = export_render(
            device.clone(),
            vk::Extent2D { width, height },
            fourcc,
            modifiers,
            placement,
        )?;
        let fence = ExportSemaphore::new(device)?;
        Ok(ExportedDmabuf {
            target: DmabufTarget {
                image: self.reclaimer.defer(exported.image),
                buffer: self.targets.new_external(),
                readable: exported.readable,
                fence: fence.map(|fence| self.reclaimer.defer(fence)),
                reclaimer: self.reclaimer.clone(),
            },
            format: exported.format,
            planes: exported.planes,
        })
    }

    pub fn import_dmabuf(&mut self, spec: &DmabufSpec<'_>) -> Result<DmabufTarget> {
        let device = self.gpu.device().clone();
        let (image, readable) = import_render(device.clone(), spec)?;
        let fence = ExportSemaphore::new(device)?;
        Ok(DmabufTarget {
            image: self.reclaimer.defer(image),
            buffer: self.targets.new_external(),
            readable,
            fence: fence.map(|fence| self.reclaimer.defer(fence)),
            reclaimer: self.reclaimer.clone(),
        })
    }
}

#[derive(Debug)]
#[must_use = "a scanout buffer is drawn onto by handing this to draw()"]
pub struct DmabufDraw<'a> {
    output: &'a mut DmabufOutput,
    buffer: &'a mut DmabufTarget,
    visible: Option<vk::Extent2D>,
    released: Option<OwnedFd>,
}

impl DmabufOutput {
    pub fn with_target<'a>(&'a mut self, buffer: &'a mut DmabufTarget) -> DmabufDraw<'a> {
        DmabufDraw {
            output: self,
            buffer,
            visible: None,
            released: None,
        }
    }
}

impl DmabufDraw<'_> {
    pub fn visible(mut self, width: u32, height: u32) -> Self {
        let whole = self.buffer.image.extent();
        self.visible = Some(vk::Extent2D {
            width: width.clamp(1, whole.width),
            height: height.clamp(1, whole.height),
        });
        self
    }

    pub fn with_release_fence(mut self, released: OwnedFd) -> Self {
        self.released = Some(released);
        self
    }
}

impl Onto for DmabufDraw<'_> {
    type Output = DmabufFrame;

    fn canvas(&mut self) -> Canvas {
        let image = &*self.buffer.image;
        let whole = image.extent();
        let plan = self
            .output
            .plan(self.visible.unwrap_or(whole))
            .onto_buffer(self.buffer.buffer)
            .read_back(self.buffer.readable);
        self.output.readable = self.buffer.readable;
        let family = image.device().queue_family();
        let acquire = match image.access() == ImageAccess::UNDEFINED {
            true => QueueTransfer::NONE,
            false => QueueTransfer::acquire(family),
        };
        let canvas = Canvas::new(plan, image.handle(), image.view(), image.format())
            .with_extent(whole)
            .with_layouts(image.access(), ImageAccess::EXTERNAL)
            .transferred(acquire, QueueTransfer::release(family));
        match self.buffer.fence.as_deref() {
            Some(fence) => canvas.signals(fence.handle(), 0, vk::PipelineStageFlags2::ALL_COMMANDS),
            None => canvas,
        }
    }

    fn released_by(&mut self) -> Option<OwnedFd> {
        self.released.take()
    }

    fn undrawn(&mut self, output: RenderOutput) -> DmabufFrame {
        DmabufFrame {
            output,
            fence: None,
        }
    }

    fn drawn(&mut self, mut painted: Painted<'_>) -> Result<DmabufFrame> {
        self.buffer.image.set_access(ImageAccess::EXTERNAL);
        let fence = match self.buffer.fence.as_deref() {
            Some(fence) => fence.export()?.map(DmabufFence::SyncFile),
            None => Some(DmabufFence::Readable(painted.readable_when_done()?)),
        };
        Ok(DmabufFrame {
            output: painted.output,
            fence,
        })
    }
}
