use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ash::vk;
use rikka_sync::Mutex;

use crate::compute::format::ImageFormat;
use crate::compute::image::ComputeImage;
use crate::compute::ticket::Ticket;
use crate::error::{Error, Result};

struct ReadyJoin {
    remaining: AtomicUsize,
    notify: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl ReadyJoin {
    fn arrive(&self) {
        if self.remaining.fetch_sub(1, Ordering::AcqRel) == 1
            && let Some(notify) = self.notify.lock().take()
        {
            notify();
        }
    }
}

#[derive(Clone, Debug)]
pub struct ComputedCopy {
    image: ComputeImage,
    ready: Ticket,
    origin: [u32; 2],
    extent: [u32; 2],
}

impl ComputedCopy {
    pub fn whole(image: &ComputeImage, ready: &Ticket) -> Result<ComputedCopy> {
        ComputedCopy::part(image, ready, [0, 0], image.extent())
    }

    pub fn part(
        image: &ComputeImage,
        ready: &Ticket,
        origin: [u32; 2],
        extent: [u32; 2],
    ) -> Result<ComputedCopy> {
        if image.format() != ImageFormat::Rgba8 {
            return Err(Error::Invalid(format!(
                "only premultiplied Rgba8 images can be presented, got {:?}",
                image.format()
            )));
        }
        let whole = image.extent();
        let inside = extent[0] > 0
            && extent[1] > 0
            && origin[0].saturating_add(extent[0]) <= whole[0]
            && origin[1].saturating_add(extent[1]) <= whole[1];
        if !inside || !image.belongs_to(&ready.shared) {
            return Err(Error::Invalid(format!(
                "{}x{} region at {},{} is outside the {}x{} image of this ticket",
                extent[0], extent[1], origin[0], origin[1], whole[0], whole[1]
            )));
        }
        Ok(ComputedCopy {
            image: image.clone(),
            ready: ready.clone(),
            origin,
            extent,
        })
    }

    #[must_use]
    pub fn extent(&self) -> [u32; 2] {
        self.extent
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready.is_ready()
    }

    pub(crate) fn wait(&self) -> vk::SemaphoreSubmitInfo<'static> {
        vk::SemaphoreSubmitInfo::default()
            .semaphore(self.ready.semaphore())
            .value(self.ready.value())
            .stage_mask(vk::PipelineStageFlags2::COPY)
    }

    pub(crate) fn source(&self) -> vk::Image {
        self.image.handle()
    }

    pub(crate) fn origin(&self) -> [u32; 2] {
        self.origin
    }
}

#[derive(Clone, Debug)]
pub struct ComputedFrame {
    width: u32,
    height: u32,
    parts: Vec<([u32; 2], ComputedCopy)>,
}

impl ComputedFrame {
    pub fn new(
        width: u32,
        height: u32,
        parts: Vec<([u32; 2], ComputedCopy)>,
    ) -> Result<ComputedFrame> {
        let fits = parts.iter().all(|(at, copy)| {
            let [w, h] = copy.extent();
            at[0].saturating_add(w) <= width && at[1].saturating_add(h) <= height
        });
        if width == 0 || height == 0 || !fits {
            return Err(Error::Invalid(format!(
                "tiles do not cover a {width}x{height} image"
            )));
        }
        Ok(ComputedFrame {
            width,
            height,
            parts,
        })
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn parts(&self) -> &[([u32; 2], ComputedCopy)] {
        &self.parts
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.parts.iter().all(|(_, copy)| copy.is_ready())
    }

    pub fn on_ready(&self, notify: impl FnOnce() + Send + 'static) {
        let join = Arc::new(ReadyJoin {
            remaining: AtomicUsize::new(self.parts.len() + 1),
            notify: Mutex::new(Some(Box::new(notify))),
        });
        for (_, copy) in &self.parts {
            let join = Arc::clone(&join);
            copy.ready.on_ready(move || join.arrive());
        }
        join.arrive();
    }

    pub fn area(&self, origin: [u32; 2], extent: [u32; 2]) -> Result<ComputedFrame> {
        let end = [
            origin[0].saturating_add(extent[0]),
            origin[1].saturating_add(extent[1]),
        ];
        if end[0] > self.width || end[1] > self.height {
            return Err(Error::Invalid(format!(
                "{}x{} region at {},{} exceeds a {}x{} image",
                extent[0], extent[1], origin[0], origin[1], self.width, self.height
            )));
        }
        let parts = self
            .parts
            .iter()
            .filter_map(|(at, copy)| {
                let low = [at[0].max(origin[0]), at[1].max(origin[1])];
                let high = [
                    (at[0] + copy.extent[0]).min(end[0]),
                    (at[1] + copy.extent[1]).min(end[1]),
                ];
                (low[0] < high[0] && low[1] < high[1]).then(|| {
                    let part = ComputedCopy {
                        origin: [
                            copy.origin[0] + low[0] - at[0],
                            copy.origin[1] + low[1] - at[1],
                        ],
                        extent: [high[0] - low[0], high[1] - low[1]],
                        ..copy.clone()
                    };
                    ([low[0] - origin[0], low[1] - origin[1]], part)
                })
            })
            .collect();
        ComputedFrame::new(extent[0], extent[1], parts)
    }
}
