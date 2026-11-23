use std::any::Any;
use std::sync::Arc;

use ash::vk;
use rustc_hash::FxHashMap;

use crate::core::convert::{ConvertPlane, ConvertPush, YcbcrConverter};
use crate::core::device::RenderDevice;
use crate::core::frame_ctx::GpuFrameCtx;
use crate::core::image::GpuImage;
use crate::error::{Error, Result};
use crate::texture::{TextureId, TexturePool};
use crate::video::frame::{ExternalVideoFrame, WAIT_STAGE};
use crate::video::lease::FrameClaim;

struct VideoJob {
    texture: TextureId,
    frame: ExternalVideoFrame,
}

pub struct VideoImports {
    device: Arc<RenderDevice>,
    filters: FxHashMap<vk::Format, bool>,
    pending: Vec<VideoJob>,
    recorded: Vec<VideoJob>,
    handovers: Vec<vk::ImageMemoryBarrier2<'static>>,
}

#[derive(Default)]
pub struct VideoSemaphores {
    pub waits: Vec<vk::SemaphoreSubmitInfo<'static>>,
    pub signals: Vec<vk::SemaphoreSubmitInfo<'static>>,
}

impl VideoSemaphores {
    pub(crate) fn clear(&mut self) {
        self.waits.clear();
        self.signals.clear();
    }

    pub(crate) fn settle(&mut self, claim: &FrameClaim) {
        for &(semaphore, value) in &claim.waits {
            name(&mut self.waits, semaphore, value, WAIT_STAGE);
        }
        for &(semaphore, value) in &claim.signals {
            name(
                &mut self.signals,
                semaphore,
                value,
                vk::PipelineStageFlags2::ALL_COMMANDS,
            );
        }
    }
}

fn name(
    list: &mut Vec<vk::SemaphoreSubmitInfo<'static>>,
    semaphore: vk::Semaphore,
    value: u64,
    stage: vk::PipelineStageFlags2,
) {
    if let Some(named) = list.iter_mut().find(|info| info.semaphore == semaphore) {
        named.value = named.value.max(value);
        return;
    }
    list.push(
        vk::SemaphoreSubmitInfo::default()
            .semaphore(semaphore)
            .value(value)
            .stage_mask(stage),
    );
}

impl VideoImports {
    pub fn new(device: Arc<RenderDevice>) -> Self {
        Self {
            device,
            filters: FxHashMap::default(),
            pending: Vec::new(),
            recorded: Vec::new(),
            handovers: Vec::new(),
        }
    }

    pub(crate) fn well_formed(frame: &ExternalVideoFrame) -> Result<()> {
        if frame.planes.is_empty() || frame.planes.len() > 3 {
            return Err(Error::Invalid(format!(
                "video frame with {} planes",
                frame.planes.len(),
            )));
        }
        Ok(())
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn submit(&mut self, texture: TextureId, frame: ExternalVideoFrame) {
        self.pending.retain(|job| job.texture != texture);
        self.pending.push(VideoJob { texture, frame });
    }

    pub fn forget_texture(&mut self, texture: TextureId) {
        self.pending.retain(|job| job.texture != texture);
    }

    pub fn record(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        textures: &mut TexturePool,
        converter: &mut YcbcrConverter,
    ) -> Result<VideoSemaphores> {
        let mut semaphores = VideoSemaphores::default();
        if !self.has_pending() || !converter.ready() {
            return Ok(semaphores);
        }

        ctx.begin_pass("rikka.video");
        let family = ctx.device.queue_family();
        let mut handovers = std::mem::take(&mut self.handovers);
        let mut jobs = std::mem::take(&mut self.pending).into_iter();
        while let Some(mut job) = jobs.next() {
            let Some(target) = textures.image_mut(job.texture) else {
                continue;
            };
            handovers.clear();
            handovers.extend(
                job.frame
                    .lease
                    .claim()
                    .images
                    .iter()
                    .filter_map(|image| image.handover(family)),
            );
            let converted = self.convert(ctx, converter, &handovers, target, &job.frame);
            if let Err(err) = converted {
                self.handovers = handovers;
                self.pending.extend(jobs);
                return Err(err);
            }
            self.recorded.push(job);
        }
        self.handovers = handovers;
        ctx.end_pass();

        for job in &self.recorded {
            let claim = job.frame.lease.claimed();
            semaphores.settle(claim.expect("recorded frames are claimed"));
        }
        Ok(semaphores)
    }

    pub fn commit(&mut self) -> impl Iterator<Item = Box<dyn Any + Send>> + '_ {
        self.recorded.drain(..).map(|job| job.frame.lease.settle())
    }

    pub fn revert(&mut self) {
        for mut job in std::mem::take(&mut self.recorded) {
            if self
                .pending
                .iter()
                .any(|pending| pending.texture == job.texture)
            {
                continue;
            }
            job.frame.lease.unclaim();
            self.pending.push(job);
        }
    }

    fn convert(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        converter: &mut YcbcrConverter,
        handovers: &[vk::ImageMemoryBarrier2<'static>],
        target: &mut GpuImage,
        frame: &ExternalVideoFrame,
    ) -> Result<()> {
        let views = PlaneViews::create(&self.device, frame)?;
        let last = frame.planes.len() - 1;
        let planes: [ConvertPlane; 3] = std::array::from_fn(|slot| {
            let index = slot.min(last);
            ConvertPlane {
                view: views.views[index],
                linear: self.filters_linearly(frame.planes[index].view_format),
            }
        });

        let push = ConvertPush {
            rows: frame.color_matrix,
            uv_scale: frame.uv_scale,
            chroma_packed: frame.chroma_packed,
            chroma_swapped: false,
            chroma_offset: [0.0; 2],
        };
        let recorded = converter.record(ctx, handovers, target, planes, push);
        ctx.retire.defer(ctx.frame_value, Box::new(views));
        recorded
    }

    fn filters_linearly(&mut self, format: vk::Format) -> bool {
        let device = &self.device;
        *self.filters.entry(format).or_insert_with(|| {
            let properties = unsafe {
                device
                    .instance()
                    .handle()
                    .get_physical_device_format_properties(device.physical(), format)
            };
            properties
                .optimal_tiling_features
                .contains(vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR)
        })
    }
}

struct PlaneViews {
    device: Arc<RenderDevice>,
    views: Vec<vk::ImageView>,
}

impl PlaneViews {
    fn create(device: &Arc<RenderDevice>, frame: &ExternalVideoFrame) -> Result<Self> {
        let mut created = Self {
            device: device.clone(),
            views: Vec::with_capacity(frame.planes.len()),
        };
        for plane in &frame.planes {
            let mut usage =
                vk::ImageViewUsageCreateInfo::default().usage(vk::ImageUsageFlags::SAMPLED);
            let info = vk::ImageViewCreateInfo::default()
                .image(plane.image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(plane.view_format)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(plane.aspect)
                        .level_count(1)
                        .layer_count(1),
                )
                .push_next(&mut usage);
            let view = unsafe { device.handle().create_image_view(&info, None)? };
            device.set_object_name(view, "rikka.video.plane");
            created.views.push(view);
        }
        Ok(created)
    }
}

impl Drop for PlaneViews {
    fn drop(&mut self) {
        for view in self.views.drain(..) {
            unsafe { self.device.handle().destroy_image_view(view, None) };
        }
    }
}

impl std::fmt::Debug for VideoImports {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoImports").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for VideoSemaphores {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoSemaphores").finish_non_exhaustive()
    }
}
