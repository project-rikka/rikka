use std::any::Any;
use std::ops::Range;
use std::sync::Arc;

use ash::vk;
use rikka_sync::Mutex;

use crate::compute::context::Shared;
use crate::compute::lane::{Lane, Marks, Uses};
use crate::core::device::RenderDevice;
#[cfg(unix)]
use crate::core::image::GpuImage;
use crate::core::image::{ImageAccess, QueueTransfer, color_subresource_range, image_barrier};
#[cfg(unix)]
use crate::drm::{DmabufSpec, TextureLayout, import_color, texture_format};
use crate::error::{Error, Result, Store};
use crate::video::{VideoFrameImage, VideoPlane};

#[cfg(unix)]
const COMPUTE_READ: ImageAccess = ImageAccess {
    stage: vk::PipelineStageFlags2::COMPUTE_SHADER,
    access: vk::AccessFlags2::SHADER_SAMPLED_READ,
    layout: vk::ImageLayout::GENERAL,
};

const COMPUTE_WRITE: ImageAccess = ImageAccess {
    stage: vk::PipelineStageFlags2::COMPUTE_SHADER,
    access: vk::AccessFlags2::SHADER_STORAGE_WRITE,
    layout: vk::ImageLayout::GENERAL,
};

const VIDEO_WAIT_STAGE: vk::PipelineStageFlags2 = vk::PipelineStageFlags2::COMPUTE_SHADER;

const MAX_PLANES: usize = 3;

pub struct VideoTarget {
    pub extent: [u32; 2],
    pub planes: Vec<VideoPlane>,
    pub images: Vec<VideoFrameImage>,
    pub waits: Vec<(vk::Semaphore, u64)>,
    pub signals: Vec<(vk::Semaphore, u64)>,
    pub keepalive: Box<dyn Any + Send>,
}

impl std::fmt::Debug for VideoTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoTarget")
            .field("extent", &self.extent)
            .field("planes", &self.planes)
            .field("images", &self.images.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct ForeignImage {
    pub(crate) inner: Arc<Foreign>,
}

pub(crate) struct Foreign {
    shared: Arc<Shared>,
    id: u64,
    slots: Range<u32>,
    extent: [u32; 2],
    body: Option<Body>,
    uses: Uses,
}

enum Body {
    #[cfg(unix)]
    Dmabuf(GpuImage),
    Video(BorrowedVideo),
}

struct BorrowedVideo {
    device: Arc<RenderDevice>,
    views: Vec<vk::ImageView>,
    images: Vec<VideoFrameImage>,
    sync: Mutex<Option<QueueSync>>,
    _keepalive: Mutex<Box<dyn Any + Send>>,
}

struct QueueSync {
    waits: Vec<(vk::Semaphore, u64)>,
    signals: Vec<(vk::Semaphore, u64)>,
}

#[derive(Default)]
pub(crate) struct ForeignAcquire {
    pub(crate) barriers: Vec<vk::ImageMemoryBarrier2<'static>>,
    pub(crate) waits: Vec<vk::SemaphoreSubmitInfo<'static>>,
    pub(crate) signals: Vec<vk::SemaphoreSubmitInfo<'static>>,
}

impl ForeignImage {
    #[cfg(unix)]
    pub(crate) fn import_dmabuf(shared: &Arc<Shared>, spec: &DmabufSpec<'_>) -> Result<Self> {
        let format = texture_format(spec.fourcc)
            .filter(|format| matches!(format.layout, TextureLayout::Color { .. }))
            .ok_or_else(|| {
                Error::Unsupported(format!(
                    "DRM format {:#010x} is not supported for compute sampling",
                    spec.fourcc
                ))
            })?;
        let image = import_color(shared.device.clone(), spec, format)?;
        let slot = take_slots(shared, 1)?;
        shared.table.write_sampled(slot, image.view());
        Ok(Self::from_slots(
            shared,
            slot..slot + 1,
            [spec.width, spec.height],
            Body::Dmabuf(image),
        ))
    }

    pub(crate) fn video(shared: &Arc<Shared>, target: VideoTarget) -> Result<Self> {
        let VideoTarget {
            extent,
            planes,
            images,
            waits,
            signals,
            keepalive,
        } = target;
        if planes.is_empty() || planes.len() > MAX_PLANES || images.is_empty() {
            return Err(Error::Invalid(format!(
                "video target with {} planes over {} images",
                planes.len(),
                images.len()
            )));
        }
        if let Some(stray) = planes
            .iter()
            .find(|plane| !images.iter().any(|bound| bound.image == plane.image))
        {
            return Err(Error::Invalid(format!(
                "plane {:?} references an image outside the video target",
                stray.aspect
            )));
        }
        let device = shared.device.clone();
        let mut borrowed = BorrowedVideo {
            device: device.clone(),
            views: Vec::with_capacity(planes.len()),
            images,
            sync: Mutex::new(Some(QueueSync { waits, signals })),
            _keepalive: Mutex::new(keepalive),
        };
        for plane in &planes {
            borrowed.views.push(storage_view(&device, plane)?);
        }
        let count = planes.len() as u32;
        let start = take_slots(shared, count)?;
        for (slot, view) in (start..).zip(&borrowed.views) {
            shared.table.write_storage(slot, *view);
        }
        Ok(Self::from_slots(
            shared,
            start..start + count,
            extent,
            Body::Video(borrowed),
        ))
    }

    fn from_slots(shared: &Arc<Shared>, slots: Range<u32>, extent: [u32; 2], body: Body) -> Self {
        ForeignImage {
            inner: Arc::new(Foreign {
                shared: Arc::clone(shared),
                id: shared.allocate_id(),
                slots,
                extent,
                body: Some(body),
                uses: Uses::default(),
            }),
        }
    }

    #[must_use]
    pub fn slot(&self) -> u32 {
        self.inner.slots.start
    }

    #[must_use]
    pub fn plane_slot(&self, plane: usize) -> Option<u32> {
        let slot = self.inner.slots.start + u32::try_from(plane).ok()?;
        (slot < self.inner.slots.end).then_some(slot)
    }

    #[must_use]
    pub fn extent(&self) -> [u32; 2] {
        self.inner.extent
    }

    pub(crate) fn id(&self) -> u64 {
        self.inner.id
    }

    pub(crate) fn belongs_to(&self, shared: &Arc<Shared>) -> bool {
        Arc::ptr_eq(&self.inner.shared, shared)
    }

    pub(crate) fn used_until(&self, lane: Lane, value: u64) {
        self.inner.uses.mark(lane, value);
    }

    pub(crate) fn open(&self, family: u32, opening: &mut ForeignAcquire) -> Result<()> {
        match self.inner.body.as_ref() {
            #[cfg(unix)]
            Some(Body::Dmabuf(image)) => {
                opening.barriers.push(image_barrier(
                    image.handle(),
                    ImageAccess::EXTERNAL,
                    COMPUTE_READ,
                    QueueTransfer::acquire(family),
                    color_subresource_range(),
                ));
                Ok(())
            }
            Some(Body::Video(borrowed)) => {
                let sync = borrowed.sync.lock().take().ok_or_else(|| {
                    Error::Invalid("video target already written by another batch".into())
                })?;
                for frame_image in &borrowed.images {
                    let queues = match frame_image.source_queue_family {
                        Some(source) if source != family => QueueTransfer {
                            src: source,
                            dst: family,
                        },
                        _ => QueueTransfer::NONE,
                    };
                    let lent_as = ImageAccess {
                        stage: VIDEO_WAIT_STAGE,
                        access: vk::AccessFlags2::NONE,
                        layout: frame_image.layout,
                    };
                    opening.barriers.push(image_barrier(
                        frame_image.image,
                        lent_as,
                        COMPUTE_WRITE,
                        queues,
                        color_subresource_range(),
                    ));
                }
                for (semaphore, value) in sync.waits {
                    opening.waits.push(
                        vk::SemaphoreSubmitInfo::default()
                            .semaphore(semaphore)
                            .value(value)
                            .stage_mask(VIDEO_WAIT_STAGE),
                    );
                }
                for (semaphore, value) in sync.signals {
                    opening.signals.push(
                        vk::SemaphoreSubmitInfo::default()
                            .semaphore(semaphore)
                            .value(value)
                            .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS),
                    );
                }
                Ok(())
            }
            None => Err(Error::Invalid("foreign image was dropped".into())),
        }
    }

    #[cfg_attr(
        not(unix),
        expect(unused_variables, reason = "only dmabufs change queue family")
    )]
    pub(crate) fn close(&self, family: u32) -> Option<vk::ImageMemoryBarrier2<'static>> {
        match self.inner.body.as_ref()? {
            #[cfg(unix)]
            Body::Dmabuf(image) => Some(image_barrier(
                image.handle(),
                COMPUTE_READ,
                ImageAccess::EXTERNAL,
                QueueTransfer::release(family),
                color_subresource_range(),
            )),
            Body::Video(_) => None,
        }
    }
}

fn take_slots(shared: &Arc<Shared>, count: u32) -> Result<u32> {
    let mut state = shared.state.lock();
    if let Some(start) = state.slots.take(count) {
        return Ok(start);
    }
    state.slots.collect(shared.finished());
    state.slots.take(count).ok_or(Error::Full(Store::Table {
        what: "foreign image",
        slots: shared.table.capacity(),
    }))
}

fn storage_view(device: &RenderDevice, plane: &VideoPlane) -> Result<vk::ImageView> {
    let mut usage = vk::ImageViewUsageCreateInfo::default().usage(vk::ImageUsageFlags::STORAGE);
    let info = vk::ImageViewCreateInfo::default()
        .image(plane.image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(plane.view_format)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: plane.aspect,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        })
        .push_next(&mut usage);
    let view = unsafe { device.handle().create_image_view(&info, None)? };
    device.set_object_name(view, "rikka.compute.video_plane");
    Ok(view)
}

impl Drop for BorrowedVideo {
    fn drop(&mut self) {
        for view in self.views.drain(..) {
            unsafe { self.device.handle().destroy_image_view(view, None) };
        }
    }
}

impl Drop for Foreign {
    fn drop(&mut self) {
        let last: Marks = self.uses.marks();
        self.shared
            .state
            .lock()
            .slots
            .release(self.slots.clone(), last);
        if let Some(body) = self.body.take() {
            self.shared.retire(last, Box::new(body));
        }
    }
}

impl std::fmt::Debug for ForeignImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self.inner.body {
            #[cfg(unix)]
            Some(Body::Dmabuf(_)) => "dmabuf",
            Some(Body::Video(_)) => "video",
            None => "gone",
        };
        f.debug_struct("ForeignImage")
            .field("kind", &kind)
            .field("slots", &self.inner.slots)
            .field("extent", &self.inner.extent)
            .finish()
    }
}
