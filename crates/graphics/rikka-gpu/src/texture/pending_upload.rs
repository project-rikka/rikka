use ash::vk;

use crate::compute::ComputedCopy;
use crate::core::image::Texels;

use super::TextureId;
use super::ring::RingSpan;

pub(super) struct PendingUpload {
    pub(super) id: TextureId,
    pub(super) offset: [u32; 2],
    pub(super) extent: [u32; 2],
    pub(super) fill: Fill,
}

pub(super) enum Fill {
    Pixels(Vec<u8>),
    Staged(RingSpan),
    Blank,
    Computed(Box<ComputedCopy>),
}

impl PendingUpload {
    pub(super) fn texels(&self) -> Texels {
        Texels::new(self.offset, self.extent)
    }

    pub(super) fn covers(&self, other: &Self) -> bool {
        self.offset[0] <= other.offset[0]
            && self.offset[1] <= other.offset[1]
            && self.offset[0] + self.extent[0] >= other.offset[0] + other.extent[0]
            && self.offset[1] + self.extent[1] >= other.offset[1] + other.extent[1]
    }

    pub(super) fn fills(&self, whole: vk::Extent2D) -> bool {
        self.offset == [0, 0] && self.extent == [whole.width, whole.height]
    }
}
