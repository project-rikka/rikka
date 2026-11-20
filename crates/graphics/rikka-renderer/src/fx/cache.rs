use std::time::Instant;

use ash::vk;
use rikka_types::{Point, Rect, RoundedRect};
use rustc_hash::FxHashMap;

use crate::fx::anchor::Anchor;
use crate::fx::pool::{MAX_OVERSIZE, area, fits_without_waste, fx_image, size_class};
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::runtime::{Recycler, UNUSED_FOR};
use rikka_gpu::core::table_image::TableImage;
use rikka_gpu::error::Result;

const SLACK: f32 = 64.0;

#[derive(Clone, Debug, PartialEq)]
pub struct BackdropCache {
    pub(crate) key: u64,
    pub(crate) stale: Vec<Rect>,
}

impl BackdropCache {
    pub fn current(key: u64) -> Self {
        Self {
            key,
            stale: Vec::new(),
        }
    }

    pub fn stale(key: u64, stale: impl IntoIterator<Item = Rect>) -> Self {
        Self {
            key,
            stale: stale.into_iter().collect(),
        }
    }
}

pub(crate) struct CachedBackdrop {
    pub table: TableImage,
    pub anchor: Anchor,
    pub recipe: u64,
    pub rounded: Option<RoundedRect>,
    pub valid: Rect,
    last_used: Instant,
}

impl CachedBackdrop {
    fn holds(&self, want: Rect, scale: f32, format: vk::Format) -> bool {
        if self.anchor.scale() != scale || self.table.image.format() != format {
            return false;
        }
        let texel = self.anchor.rect(want);
        let extent = self.table.image.extent();
        texel.min_x() >= 0.0
            && texel.min_y() >= 0.0
            && texel.max_x() <= extent.width as f32
            && texel.max_y() <= extent.height as f32
    }

    pub(crate) fn matches(
        &self,
        want: Rect,
        (scale, recipe): (f32, u64),
        format: vk::Format,
    ) -> bool {
        self.recipe == recipe && self.holds(want, scale, format) && self.valid.contains_rect(&want)
    }

    pub(crate) fn mark_used(&mut self, now: Instant) {
        self.last_used = now;
    }
}

struct Spare {
    table: TableImage,
    since: Instant,
}

#[derive(Default)]
pub(crate) struct BackdropStore {
    entries: FxHashMap<u64, CachedBackdrop>,
    spare: Vec<Spare>,
}

impl BackdropStore {
    pub(crate) fn get(&self, key: u64) -> Option<&CachedBackdrop> {
        self.entries.get(&key)
    }

    pub(crate) fn get_mut(&mut self, key: u64) -> Option<&mut CachedBackdrop> {
        self.entries.get_mut(&key)
    }

    pub(crate) fn refresh(&mut self, keys: &[u64], now: Instant) {
        for key in keys {
            if let Some(cached) = self.entries.get_mut(key) {
                cached.last_used = now;
            }
        }
    }

    pub(crate) fn invalidate(&mut self, key: u64) {
        if let Some(cached) = self.entries.get_mut(&key) {
            cached.valid = Rect::ZERO;
        }
    }

    pub(crate) fn take(&mut self, key: u64) -> Option<CachedBackdrop> {
        self.entries.remove(&key)
    }

    pub(crate) fn put(&mut self, key: u64, mut cached: CachedBackdrop, now: Instant) {
        cached.last_used = now;
        self.entries.insert(key, cached);
    }

    pub(crate) fn fit(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        cached: Option<CachedBackdrop>,
        want: Rect,
        (scale, recipe): (f32, u64),
        format: vk::Format,
        now: Instant,
    ) -> Result<CachedBackdrop> {
        let roomy = want.inflate(SLACK).round_out();
        let step = 1.0 / scale;
        let anchor = Anchor::new(
            Point::new(snap(roomy.min_x(), step), snap(roomy.min_y(), step)),
            scale,
        );
        let covered = anchor.rect(roomy);
        let extent = vk::Extent2D {
            width: span(covered.max_x()),
            height: span(covered.max_y()),
        };

        let cached = match cached {
            Some(cached) if cached.table.image.format() != format => {
                self.spare.push(Spare {
                    table: cached.table,
                    since: now,
                });
                None
            }
            cached => cached,
        };
        if let Some(mut cached) = cached {
            if cached.holds(want, scale, format)
                && area(cached.table.image.extent()) <= area(extent) * MAX_OVERSIZE
            {
                if cached.recipe != recipe {
                    cached.recipe = recipe;
                    cached.valid = Rect::ZERO;
                }
                return Ok(cached);
            }
            let current = cached.table.image.extent();
            if fits_without_waste(current, extent) {
                cached.anchor = anchor;
                cached.recipe = recipe;
                cached.valid = Rect::ZERO;
                return Ok(cached);
            }
            self.spare.push(Spare {
                table: cached.table,
                since: now,
            });
        }

        let table = match self.take_spare(extent, format) {
            Some(table) => table,
            None => fx_image(ctx, "rikka.backdrop", extent, format)?,
        };
        Ok(CachedBackdrop {
            table,
            anchor,
            recipe,
            rounded: None,
            valid: Rect::ZERO,
            last_used: now,
        })
    }

    fn take_spare(&mut self, extent: vk::Extent2D, format: vk::Format) -> Option<TableImage> {
        let index = self
            .spare
            .iter()
            .enumerate()
            .filter(|(_, spare)| {
                spare.table.image.format() == format
                    && fits_without_waste(spare.table.image.extent(), extent)
            })
            .min_by_key(|(_, spare)| area(spare.table.image.extent()))
            .map(|(index, _)| index)?;
        let mut spare = self.spare.swap_remove(index);
        spare.table.image.discard();
        Some(spare.table)
    }

    pub(crate) fn end_frame(&mut self, ctx: &mut GpuFrameCtx<'_>, now: Instant) {
        let expired = self
            .entries
            .extract_if(|_, cached| now.duration_since(cached.last_used) >= UNUSED_FOR);
        for (_, cached) in expired {
            cached.table.retire(&mut ctx.recycler());
        }
        self.trim(&mut ctx.recycler(), now);
    }

    pub(crate) fn trim(&mut self, recycler: &mut Recycler<'_>, now: Instant) {
        let mut index = 0;
        while index < self.spare.len() {
            if now.duration_since(self.spare[index].since) < UNUSED_FOR {
                index += 1;
                continue;
            }
            let spare = self.spare.swap_remove(index);
            spare.table.retire(recycler);
        }
    }
}

fn snap(value: f32, step: f32) -> f32 {
    (value / step).floor() * step
}

fn span(texels: f32) -> u32 {
    size_class(texels.max(1.0) as u32)
}
