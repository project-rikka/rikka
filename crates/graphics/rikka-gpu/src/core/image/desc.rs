use ash::vk;

use crate::core::device::RenderDevice;
use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug)]
pub struct ImageDesc {
    pub extent: vk::Extent2D,
    pub format: vk::Format,
    pub usage: vk::ImageUsageFlags,
    pub mip_levels: u32,
    pub array_layers: u32,
    pub samples: vk::SampleCountFlags,
    pub aspect: vk::ImageAspectFlags,
    pub view_type: vk::ImageViewType,
    pub shared_by: Option<[u32; 2]>,
    pub components: vk::ComponentMapping,
}

impl ImageDesc {
    pub fn color(extent: vk::Extent2D, format: vk::Format, usage: vk::ImageUsageFlags) -> Self {
        Self {
            extent,
            format,
            usage,
            mip_levels: 1,
            array_layers: 1,
            samples: vk::SampleCountFlags::TYPE_1,
            aspect: vk::ImageAspectFlags::COLOR,
            view_type: vk::ImageViewType::TYPE_2D,
            shared_by: None,
            components: vk::ComponentMapping::default(),
        }
    }

    pub fn depth(extent: vk::Extent2D, format: vk::Format, usage: vk::ImageUsageFlags) -> Self {
        Self {
            aspect: vk::ImageAspectFlags::DEPTH,
            ..Self::color(extent, format, usage)
        }
    }

    pub fn samples(mut self, samples: vk::SampleCountFlags) -> Self {
        self.samples = samples;
        self
    }

    pub fn mips(mut self, levels: u32) -> Self {
        self.mip_levels = levels.max(1);
        self
    }

    pub(crate) fn shared_by(mut self, families: Option<[u32; 2]>) -> Self {
        self.shared_by = families;
        self
    }

    pub(crate) fn components(mut self, components: vk::ComponentMapping) -> Self {
        self.components = components;
        self
    }

    pub(crate) fn create_info<'a>(&self, families: &'a [u32; 2]) -> vk::ImageCreateInfo<'a> {
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(self.format)
            .extent(vk::Extent3D {
                width: self.extent.width,
                height: self.extent.height,
                depth: 1,
            })
            .mip_levels(self.mip_levels)
            .array_layers(self.array_layers)
            .samples(self.samples)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(self.usage)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        match self.shared_by {
            Some(_) => info
                .sharing_mode(vk::SharingMode::CONCURRENT)
                .queue_family_indices(families),
            None => info.sharing_mode(vk::SharingMode::EXCLUSIVE),
        }
    }

    pub(crate) fn range(&self) -> vk::ImageSubresourceRange {
        vk::ImageSubresourceRange {
            aspect_mask: self.aspect,
            base_mip_level: 0,
            level_count: self.mip_levels,
            base_array_layer: 0,
            layer_count: self.array_layers,
        }
    }
}

pub(super) fn validate_extent(device: &RenderDevice, extent: vk::Extent2D) -> Result<()> {
    if extent.width == 0 || extent.height == 0 {
        return Err(Error::Invalid(format!(
            "image size {}x{} is empty",
            extent.width, extent.height
        )));
    }
    let largest = device.limits().max_image_dimension2_d;
    if extent.width > largest || extent.height > largest {
        return Err(Error::Unsupported(format!(
            "image size {}x{} exceeds the device limit of {largest}x{largest}",
            extent.width, extent.height
        )));
    }
    Ok(())
}
