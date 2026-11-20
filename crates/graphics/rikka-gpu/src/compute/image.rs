use std::mem::ManuallyDrop;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ash::vk;

use crate::compute::context::Shared;
use crate::compute::format::ImageFormat;
use crate::compute::lane::{Lane, Marks, Uses};
use crate::compute::pool::{PooledImage, extent_of};
use crate::core::image::{GpuImage, ImageDesc};
use crate::error::{Error, Result, Store};

#[derive(Clone)]
pub struct ComputeImage {
    pub(crate) inner: Arc<ImageBinding>,
}

#[derive(Clone, Copy, Debug)]
pub enum ComputeTile<'a> {
    New([u32; 2]),
    Reuse(&'a ComputeImage),
}

pub(crate) struct ImageBinding {
    body: Arc<Body>,
    slot: u32,
    run: Arc<Run>,
}

struct Body {
    shared: Arc<Shared>,
    image: ManuallyDrop<GpuImage>,
    format: ImageFormat,
    extent: [u32; 2],
    id: u64,
    ready: AtomicBool,
    uses: Uses,
}

struct Run {
    shared: Arc<Shared>,
    range: Range<u32>,
    uses: Uses,
}

impl ComputeImage {
    pub(crate) fn bind(
        shared: &Arc<Shared>,
        tiles: &[ComputeTile<'_>],
        format: ImageFormat,
    ) -> Result<Vec<ComputeImage>> {
        let count = u32::try_from(tiles.len())
            .map_err(|_| Error::Invalid("too many images in one batch".into()))?;
        for tile in tiles {
            if let ComputeTile::Reuse(image) = tile
                && (image.format() != format || !image.belongs_to(shared))
            {
                return Err(Error::Invalid(format!(
                    "{:?} image cannot share a pool with {format:?} images of another context",
                    image.format()
                )));
            }
        }
        let bodies = tiles
            .iter()
            .map(|tile| match tile {
                ComputeTile::New(extent) => Body::allocate(shared, *extent, format),
                ComputeTile::Reuse(image) => Ok(Arc::clone(&image.inner.body)),
            })
            .collect::<Result<Vec<_>>>()?;
        let start = ComputeImage::slots(shared, count)?;
        let run = Arc::new(Run {
            shared: Arc::clone(shared),
            range: start..start + count,
            uses: Uses::default(),
        });
        let state_guard = shared.state.lock();
        for (body, slot) in bodies.iter().zip(start..) {
            shared.table.write(slot, body.image.view());
        }
        drop(state_guard);
        Ok(bodies
            .into_iter()
            .zip(start..)
            .map(|(body, slot)| ComputeImage {
                inner: Arc::new(ImageBinding {
                    body,
                    slot,
                    run: Arc::clone(&run),
                }),
            })
            .collect())
    }

    fn slots(shared: &Arc<Shared>, count: u32) -> Result<u32> {
        let mut state = shared.state.lock();
        if let Some(start) = state.slots.take(count) {
            return Ok(start);
        }
        state.slots.collect(shared.finished());
        state.slots.take(count).ok_or(Error::Full(Store::Table {
            what: "compute image",
            slots: shared.table.capacity(),
        }))
    }

    #[must_use]
    pub fn slot(&self) -> u32 {
        self.inner.slot
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.inner.body.extent[0]
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.inner.body.extent[1]
    }

    #[must_use]
    pub fn extent(&self) -> [u32; 2] {
        self.inner.body.extent
    }

    #[must_use]
    pub fn format(&self) -> ImageFormat {
        self.inner.body.format
    }

    #[must_use]
    pub fn aliases(&self, other: &ComputeImage) -> bool {
        Arc::ptr_eq(&self.inner.body, &other.inner.body)
    }

    #[must_use]
    pub fn pixels_id(&self) -> u64 {
        self.inner.body.id
    }

    pub(crate) fn id(&self) -> u64 {
        self.inner.body.id
    }

    pub(crate) fn handle(&self) -> vk::Image {
        self.inner.body.image.handle()
    }

    pub(crate) fn range(&self) -> vk::ImageSubresourceRange {
        self.inner.body.image.subresource_range()
    }

    pub(crate) fn belongs_to(&self, shared: &Arc<Shared>) -> bool {
        Arc::ptr_eq(&self.inner.body.shared, shared)
    }

    pub(crate) fn take_first_use(&self) -> bool {
        !self.inner.body.ready.swap(true, Ordering::AcqRel)
    }

    pub(crate) fn forget_first_use(&self) {
        self.inner.body.ready.store(false, Ordering::Release);
    }

    pub(crate) fn used_until(&self, lane: Lane, value: u64) {
        self.inner.body.uses.mark(lane, value);
        self.inner.run.uses.mark(lane, value);
    }

    pub(crate) fn uses(&self) -> Marks {
        self.inner.body.uses.marks()
    }
}

impl Body {
    fn allocate(shared: &Arc<Shared>, extent: [u32; 2], format: ImageFormat) -> Result<Arc<Body>> {
        let pooled = shared
            .state
            .lock()
            .pool
            .take_image(format, extent, shared.finished());
        let (image, ready) = match pooled {
            Some(pooled) => (pooled.image, pooled.ready),
            None => {
                let usage = vk::ImageUsageFlags::STORAGE
                    | vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST;
                let desc = ImageDesc::color(
                    vk::Extent2D {
                        width: extent[0],
                        height: extent[1],
                    },
                    format.vk(),
                    usage,
                )
                .shared_by(shared.shared_by);
                let image = GpuImage::create(shared.device.clone(), "rikka.compute.image", &desc)?;
                (image, false)
            }
        };
        Ok(Arc::new(Body {
            shared: Arc::clone(shared),
            extent: extent_of(&image),
            image: ManuallyDrop::new(image),
            format,
            id: shared.allocate_id(),
            ready: AtomicBool::new(ready),
            uses: Uses::default(),
        }))
    }
}

impl Drop for Body {
    fn drop(&mut self) {
        let image = unsafe { ManuallyDrop::take(&mut self.image) };
        let pooled = PooledImage::new(
            self.format,
            image,
            self.ready.load(Ordering::Acquire),
            self.uses.marks(),
        );
        let mut state = self.shared.state.lock();
        let evicted = state.pool.recycle_image(pooled);
        state.release_eviction(evicted);
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        let last = self.uses.marks();
        self.shared
            .state
            .lock()
            .slots
            .release(self.range.clone(), last);
    }
}

impl std::fmt::Debug for ComputeImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComputeImage")
            .field("slot", &self.inner.slot)
            .field("extent", &self.inner.body.extent)
            .field("format", &self.inner.body.format)
            .finish()
    }
}
