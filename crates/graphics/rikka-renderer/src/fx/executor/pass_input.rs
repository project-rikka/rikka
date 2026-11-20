use ash::vk;
use rikka_types::{Point, Rect};

use rikka_gpu::core::table_image::TableImage;

use super::effect_output::clamped_uv;

#[derive(Clone, Copy)]
pub(crate) struct PassInput {
    pub tex: u32,
    pub texture_size: vk::Extent2D,
    pub region: Rect,
    pub origin: Point,
    pub scale: f32,
}

impl PassInput {
    pub(crate) fn from_table(table: &TableImage, region: Rect) -> Self {
        Self {
            tex: table.tex,
            texture_size: table.image.extent(),
            region,
            origin: Point::ZERO,
            scale: 1.0,
        }
    }

    pub(super) fn texels(&self) -> Rect {
        self.texels_of(self.region)
    }

    pub(super) fn uv_bounds(&self) -> [f32; 4] {
        clamped_uv(self.texels(), self.texture_size)
    }

    pub(super) fn texels_of(&self, region: Rect) -> Rect {
        Rect::new(
            (region.min_x() - self.origin.x) / self.scale,
            (region.min_y() - self.origin.y) / self.scale,
            region.width() / self.scale,
            region.height() / self.scale,
        )
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PassOutput {
    pub view: vk::ImageView,
    pub format: vk::Format,
    pub region: Rect,
}
