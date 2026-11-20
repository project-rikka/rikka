use ash::vk;

use crate::core::device::RenderDevice;
use crate::core::image::access::{ImageAccess, record_barriers};
use crate::core::image::gpu::GpuImage;

pub fn mip_levels_for(extent: vk::Extent2D) -> u32 {
    32 - extent.width.max(extent.height).max(1).leading_zeros()
}

pub(crate) fn blits_linearly(device: &RenderDevice, format: vk::Format) -> bool {
    let properties = unsafe {
        device
            .instance()
            .handle()
            .get_physical_device_format_properties(device.physical(), format)
    };
    properties.optimal_tiling_features.contains(
        vk::FormatFeatureFlags::BLIT_SRC
            | vk::FormatFeatureFlags::BLIT_DST
            | vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Texels {
    pub min: [u32; 2],
    pub max: [u32; 2],
}

impl Texels {
    pub(crate) fn new(offset: [u32; 2], extent: [u32; 2]) -> Self {
        Self {
            min: offset,
            max: [offset[0] + extent[0], offset[1] + extent[1]],
        }
    }

    pub(crate) fn union(self, other: Self) -> Self {
        Self {
            min: [self.min[0].min(other.min[0]), self.min[1].min(other.min[1])],
            max: [self.max[0].max(other.max[0]), self.max[1].max(other.max[1])],
        }
    }

    pub(super) fn below(self, extent: vk::Extent2D) -> (Self, Self) {
        let whole = [extent.width, extent.height];
        let below = whole.map(|length| (length / 2).max(1));
        let axis = |at: usize| {
            let low = (self.min[at] / 2).min(below[at] - 1);
            let high = self.max[at].div_ceil(2).clamp(low + 1, below[at]);
            let reach = match high == below[at] {
                true => whole[at],
                false => high * 2,
            };
            (low, high, reach)
        };
        let (x, y) = (axis(0), axis(1));
        let down = Self {
            min: [x.0, y.0],
            max: [x.1, y.1],
        };
        let source = Self {
            min: [x.0 * 2, y.0 * 2],
            max: [x.2, y.2],
        };
        (source, down)
    }

    pub(super) fn corners(self) -> [vk::Offset3D; 2] {
        [
            vk::Offset3D {
                x: self.min[0] as i32,
                y: self.min[1] as i32,
                z: 0,
            },
            vk::Offset3D {
                x: self.max[0] as i32,
                y: self.max[1] as i32,
                z: 1,
            },
        ]
    }
}

pub(crate) fn record_mip_chains(
    device: &RenderDevice,
    cmd: vk::CommandBuffer,
    images: &mut [(&mut GpuImage, Texels)],
    then: ImageAccess,
) {
    let deepest = images
        .iter()
        .map(|(image, _)| image.mip_levels())
        .max()
        .unwrap_or(1);
    for level in 1..deepest {
        let barriers: Vec<_> = images
            .iter()
            .filter(|(image, _)| image.mip_levels() > level)
            .map(|(image, _)| {
                image.level_barrier(
                    level - 1..level,
                    ImageAccess::TRANSFER_DST,
                    ImageAccess::TRANSFER_SRC,
                )
            })
            .collect();
        record_barriers(device, cmd, &barriers);
        for (image, texels) in images.iter_mut() {
            if image.mip_levels() > level {
                *texels = image.blit_down(cmd, level, *texels);
            }
        }
    }
    let mut barriers = Vec::with_capacity(images.len() * 2);
    for (image, _) in images.iter() {
        let last = image.mip_levels() - 1;
        if last > 0 {
            barriers.push(image.level_barrier(0..last, ImageAccess::TRANSFER_SRC, then));
        }
        barriers.push(image.level_barrier(last..last + 1, ImageAccess::TRANSFER_DST, then));
    }
    record_barriers(device, cmd, &barriers);
    for (image, _) in images.iter_mut() {
        image.settle(then);
    }
}
