use std::time::{Duration, Instant};

use crate::compute::format::ImageFormat;
use crate::compute::lane::{Marks, reached};
use crate::core::buffer::GpuBuffer;
use crate::core::image::GpuImage;

const KEPT_BYTES: u64 = 1 << 30;

const KEPT_IMAGES: usize = 1024;

const KEPT_BUFFER_BYTES: u64 = 256 << 20;

const IDLE: Duration = Duration::from_secs(2);

pub(crate) struct PooledImage {
    pub(crate) format: ImageFormat,
    pub(crate) image: GpuImage,
    pub(crate) ready: bool,
    pub(crate) last: Marks,
    since: Instant,
}

impl PooledImage {
    pub(crate) fn new(
        format: ImageFormat,
        image: GpuImage,
        ready: bool,
        last: Marks,
    ) -> PooledImage {
        PooledImage {
            format,
            image,
            ready,
            last,
            since: Instant::now(),
        }
    }

    fn bytes(&self) -> u64 {
        let [width, height] = extent_of(&self.image);
        u64::from(width) * u64::from(height) * u64::from(self.format.bytes_per_pixel())
    }

    fn fits(&self, format: ImageFormat, extent: [u32; 2]) -> bool {
        self.format == format && extent_of(&self.image) == extent
    }
}

pub(crate) fn extent_of(image: &GpuImage) -> [u32; 2] {
    let extent = image.extent();
    [extent.width, extent.height]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Use {
    Staging,
    DownloadSlot,
}

struct PooledBuffer {
    buffer: GpuBuffer,
    used: Use,
    last: Marks,
    since: Instant,
}

#[derive(Default)]
pub(crate) struct Pool {
    images: Vec<PooledImage>,
    image_bytes: u64,
    buffers: Vec<PooledBuffer>,
    buffer_bytes: u64,
}

#[derive(Default)]
pub(crate) struct Eviction {
    pub(crate) images: Vec<PooledImage>,
    pub(crate) buffers: Vec<(Marks, GpuBuffer)>,
}

impl Pool {
    pub(crate) fn take_image(
        &mut self,
        format: ImageFormat,
        extent: [u32; 2],
        completed: Marks,
    ) -> Option<PooledImage> {
        let found = self
            .images
            .iter()
            .rposition(|pooled| reached(pooled.last, completed) && pooled.fits(format, extent))?;
        let pooled = self.images.remove(found);
        self.image_bytes -= pooled.bytes();
        Some(pooled)
    }

    pub(crate) fn recycle_image(&mut self, pooled: PooledImage) -> Eviction {
        self.image_bytes += pooled.bytes();
        self.images.push(pooled);
        let mut evicted = Eviction::default();
        while self.images.len() > KEPT_IMAGES || self.image_bytes > KEPT_BYTES {
            let oldest = self.images.remove(0);
            self.image_bytes -= oldest.bytes();
            evicted.images.push(oldest);
        }
        evicted
    }

    pub(crate) fn take_buffer(
        &mut self,
        used: Use,
        size: u64,
        completed: Marks,
    ) -> Option<GpuBuffer> {
        let found = self
            .buffers
            .iter()
            .enumerate()
            .filter(|(_, pooled)| {
                pooled.used == used
                    && reached(pooled.last, completed)
                    && pooled.buffer.size() >= size
                    && pooled.buffer.size() <= fitting(size).saturating_mul(2)
            })
            .min_by_key(|(_, pooled)| pooled.buffer.size())
            .map(|(at, _)| at)?;
        let pooled = self.buffers.swap_remove(found);
        self.buffer_bytes -= pooled.buffer.size();
        Some(pooled.buffer)
    }

    pub(crate) fn recycle_buffer(&mut self, used: Use, buffer: GpuBuffer, last: Marks) -> Eviction {
        self.buffer_bytes += buffer.size();
        self.buffers.push(PooledBuffer {
            buffer,
            used,
            last,
            since: Instant::now(),
        });
        let mut evicted = Eviction::default();
        while self.buffer_bytes > KEPT_BUFFER_BYTES {
            let oldest = self.buffers.remove(0);
            self.buffer_bytes -= oldest.buffer.size();
            evicted.buffers.push((oldest.last, oldest.buffer));
        }
        evicted
    }

    pub(crate) fn trim_idle(&mut self) -> Eviction {
        let now = Instant::now();
        self.trim(|since| now.duration_since(since) >= IDLE)
    }

    pub(crate) fn clear(&mut self) -> Eviction {
        self.trim(|_| true)
    }

    fn trim(&mut self, gone: impl Fn(Instant) -> bool) -> Eviction {
        let mut evicted = Eviction::default();
        let (old, pooled): (Vec<_>, Vec<_>) = std::mem::take(&mut self.images)
            .into_iter()
            .partition(|pooled| gone(pooled.since));
        self.images = pooled;
        self.image_bytes -= old.iter().map(PooledImage::bytes).sum::<u64>();
        evicted.images = old;
        let (old, pooled): (Vec<_>, Vec<_>) = std::mem::take(&mut self.buffers)
            .into_iter()
            .partition(|pooled| gone(pooled.since));
        self.buffers = pooled;
        for pooled in old {
            self.buffer_bytes -= pooled.buffer.size();
            evicted.buffers.push((pooled.last, pooled.buffer));
        }
        evicted
    }
}

pub(crate) fn fitting(size: u64) -> u64 {
    const GRAIN: u64 = 64 << 10;
    size.max(1).next_multiple_of(GRAIN)
}
