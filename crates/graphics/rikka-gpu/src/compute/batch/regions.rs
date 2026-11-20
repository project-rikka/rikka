use ash::vk;

use crate::compute::buffer::ComputeBuffer;
use crate::core::image::color_subresource_layers;
use crate::error::Error;

pub(super) fn fits(whole: [u32; 2], origin: [u32; 2], extent: [u32; 2]) -> bool {
    extent[0] > 0
        && extent[1] > 0
        && origin[0]
            .checked_add(extent[0])
            .is_some_and(|end| end <= whole[0])
        && origin[1]
            .checked_add(extent[1])
            .is_some_and(|end| end <= whole[1])
}

pub(super) fn outside(whole: [u32; 2], origin: [u32; 2], extent: [u32; 2]) -> Error {
    Error::Invalid(format!(
        "{}x{} region at {},{} exceeds a {}x{} image",
        extent[0], extent[1], origin[0], origin[1], whole[0], whole[1]
    ))
}

pub(super) fn within(buffer: &ComputeBuffer, at: u64, size: u64) -> bool {
    size > 0 && at.checked_add(size).is_some_and(|end| end <= buffer.size())
}

pub(super) fn outside_buffer(buffer: &ComputeBuffer, at: u64, size: u64) -> Error {
    Error::Invalid(format!(
        "{size} bytes at offset {at} exceed a buffer of {} bytes",
        buffer.size()
    ))
}

pub(super) fn foreign(what: &str) -> Error {
    Error::Invalid(format!("{what} belongs to another compute context"))
}

pub(super) fn offset(origin: [u32; 2]) -> vk::Offset3D {
    vk::Offset3D {
        x: i32::try_from(origin[0]).unwrap_or(i32::MAX),
        y: i32::try_from(origin[1]).unwrap_or(i32::MAX),
        z: 0,
    }
}

pub(super) fn extent3(extent: [u32; 2]) -> vk::Extent3D {
    vk::Extent3D {
        width: extent[0],
        height: extent[1],
        depth: 1,
    }
}

pub(super) fn buffer_image_region(origin: [u32; 2], extent: [u32; 2]) -> vk::BufferImageCopy {
    vk::BufferImageCopy::default()
        .image_subresource(color_subresource_layers())
        .image_offset(offset(origin))
        .image_extent(extent3(extent))
}
