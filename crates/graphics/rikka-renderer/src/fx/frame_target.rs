use ash::vk;
use rikka_colorimetry::BlendSpace;
use rikka_types::{Rect, Size, Transform};

use rikka_gpu::core::image::{ImageAccess, QueueTransfer, color_subresource_range, image_barrier};

#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameTarget {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub format: vk::Format,
    pub texture: vk::Extent2D,
    pub turn: Transform,
    pub scene: Rect,
    pub space: BlendSpace,
}

impl FrameTarget {
    pub(crate) fn texels(&self, rect: Rect) -> Rect {
        self.turn
            .map_rect(rect, Size::new(self.scene.width(), self.scene.height()))
    }

    pub(crate) fn barrier(
        &self,
        from: ImageAccess,
        to: ImageAccess,
    ) -> vk::ImageMemoryBarrier2<'static> {
        image_barrier(
            self.image,
            from,
            to,
            QueueTransfer::NONE,
            color_subresource_range(),
        )
    }
}
