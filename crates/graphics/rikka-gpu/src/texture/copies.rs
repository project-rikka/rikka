use ash::vk;

use crate::compute::ComputedCopy;
use crate::core::image::{
    GpuImage, ImageAccess, QueueTransfer, Texels, color_subresource_layers, image_barrier,
};

use super::writes::CopySource;

pub(super) fn copy_wave(earlier: &[(Texels, usize)], texels: Texels) -> usize {
    earlier
        .iter()
        .filter(|(placed, _)| overlaps(*placed, texels))
        .map(|(_, wave)| wave + 1)
        .max()
        .unwrap_or(0)
}

fn overlaps(a: Texels, b: Texels) -> bool {
    a.min[0] < b.max[0] && b.min[0] < a.max[0] && a.min[1] < b.max[1] && b.min[1] < a.max[1]
}

pub(super) fn whole_region(extent: vk::Extent2D) -> vk::ImageCopy {
    vk::ImageCopy::default()
        .src_subresource(color_subresource_layers())
        .dst_subresource(color_subresource_layers())
        .extent(vk::Extent3D {
            width: extent.width,
            height: extent.height,
            depth: 1,
        })
}

pub(super) fn computed_region(
    copy: &ComputedCopy,
    offset: [u32; 2],
    extent: [u32; 2],
) -> vk::ImageCopy {
    let [x, y] = copy.origin();
    vk::ImageCopy::default()
        .src_subresource(color_subresource_layers())
        .src_offset(vk::Offset3D {
            x: x as i32,
            y: y as i32,
            z: 0,
        })
        .dst_subresource(color_subresource_layers())
        .dst_offset(vk::Offset3D {
            x: offset[0] as i32,
            y: offset[1] as i32,
            z: 0,
        })
        .extent(vk::Extent3D {
            width: extent[0],
            height: extent[1],
            depth: 1,
        })
}

pub(super) fn record_copies(
    device: &ash::Device,
    cmd: vk::CommandBuffer,
    image: vk::Image,
    group: &[&CopySource],
) {
    match group {
        [CopySource::Staged(buffer, _), ..] => {
            let regions: Vec<vk::BufferImageCopy> = group
                .iter()
                .filter_map(|copy| match copy {
                    CopySource::Staged(_, region) => Some(*region),
                    CopySource::Image(..) => None,
                })
                .collect();
            unsafe {
                device.cmd_copy_buffer_to_image(
                    cmd,
                    *buffer,
                    image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &regions,
                )
            };
        }
        _ => {
            for copy in group {
                if let CopySource::Image(source, layout, region) = copy {
                    unsafe {
                        device.cmd_copy_image(
                            cmd,
                            *source,
                            *layout,
                            image,
                            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                            std::slice::from_ref(region),
                        )
                    };
                }
            }
        }
    }
}

pub(super) fn wait_on(
    waits: &mut Vec<vk::SemaphoreSubmitInfo<'static>>,
    wait: vk::SemaphoreSubmitInfo<'static>,
) {
    match waits
        .iter_mut()
        .find(|named| named.semaphore == wait.semaphore)
    {
        Some(named) => named.value = named.value.max(wait.value),
        None => waits.push(wait),
    }
}

pub(super) fn copy_region(
    at: vk::DeviceSize,
    offset: [u32; 2],
    extent: [u32; 2],
) -> vk::BufferImageCopy {
    vk::BufferImageCopy::default()
        .buffer_offset(at)
        .image_subresource(color_subresource_layers())
        .image_offset(vk::Offset3D {
            x: offset[0] as i32,
            y: offset[1] as i32,
            z: 0,
        })
        .image_extent(vk::Extent3D {
            width: extent[0],
            height: extent[1],
            depth: 1,
        })
}

pub(super) fn rewrite_barrier(image: &GpuImage) -> vk::ImageMemoryBarrier2<'static> {
    let dst = ImageAccess::TRANSFER_DST;
    image_barrier(
        image.handle(),
        dst,
        dst,
        QueueTransfer::NONE,
        image.subresource_range(),
    )
}
