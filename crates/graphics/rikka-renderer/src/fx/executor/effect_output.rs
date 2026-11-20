use ash::vk;
use rikka_types::{Point, Rect};

use crate::fx::anchor::Anchor;
use crate::fx::cache::CachedBackdrop;

#[derive(Clone, Copy, Debug)]
pub(crate) struct EffectOutput {
    pub(super) pool_index: Option<usize>,
    pub tex: u32,
    pub(super) anchor: Anchor,
    pub(super) texture_size: vk::Extent2D,
    pub(super) bounds: Rect,
}

impl EffectOutput {
    pub(crate) fn uv_of(&self, rect: Rect) -> [f32; 4] {
        let min = self.anchor.point(Point::new(rect.min_x(), rect.min_y()));
        let max = self.anchor.point(Point::new(rect.max_x(), rect.max_y()));
        self.normalized(min, max)
    }

    pub(crate) fn uv_bounds(&self) -> [f32; 4] {
        clamped_uv(self.bounds, self.texture_size)
    }

    fn normalized(&self, min: Point, max: Point) -> [f32; 4] {
        let width = self.texture_size.width as f32;
        let height = self.texture_size.height as f32;
        [min.x / width, min.y / height, max.x / width, max.y / height]
    }
}

pub(super) fn clamped_uv(bounds: Rect, texture_size: vk::Extent2D) -> [f32; 4] {
    let inv_w = 1.0 / texture_size.width as f32;
    let inv_h = 1.0 / texture_size.height as f32;
    let center = bounds.center();
    [
        (bounds.min_x() + 0.5).min(center.x) * inv_w,
        (bounds.min_y() + 0.5).min(center.y) * inv_h,
        (bounds.max_x() - 0.5).max(center.x) * inv_w,
        (bounds.max_y() - 0.5).max(center.y) * inv_h,
    ]
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Capture {
    pub(super) pool_index: usize,
    pub(super) texels: Rect,
}

pub(super) fn cached_output(cached: &CachedBackdrop, want: Rect) -> EffectOutput {
    EffectOutput {
        pool_index: None,
        tex: cached.table.tex,
        anchor: cached.anchor,
        texture_size: cached.table.image.extent(),
        bounds: cached.anchor.rect(want),
    }
}
