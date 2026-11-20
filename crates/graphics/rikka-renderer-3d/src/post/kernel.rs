use ash::vk;

use crate::graph::groups;

const GROUP: u32 = 8;

pub(super) fn covering(extent: vk::Extent2D) -> (u32, u32) {
    groups(extent, GROUP)
}

pub(super) fn size_of_extent(extent: vk::Extent2D) -> [u32; 2] {
    [extent.width, extent.height]
}

pub(super) fn texel_of(extent: vk::Extent2D) -> [f32; 2] {
    [1.0 / extent.width as f32, 1.0 / extent.height as f32]
}

pub(super) fn extent_of(extent: vk::Extent2D) -> [f32; 2] {
    [extent.width as f32, extent.height as f32]
}
