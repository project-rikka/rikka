use std::sync::Arc;

use ash::vk;

use crate::core::device::RenderDevice;
use crate::core::image::{GpuImage, ImageDesc, blits_linearly, mip_levels_for};
use crate::error::Result;

use super::book::Mips;
use super::{CANVAS_USAGE, PICTURE_USAGE};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Use {
    Picture,
    Canvas,
}

impl Use {
    pub(super) fn image(
        self,
        device: &Arc<RenderDevice>,
        extent: vk::Extent2D,
        format: vk::Format,
    ) -> Result<GpuImage> {
        self.image_as(device, ImageDesc::color(extent, format, self.usage()))
    }

    pub(super) fn image_as(self, device: &Arc<RenderDevice>, desc: ImageDesc) -> Result<GpuImage> {
        GpuImage::create(device.clone(), "rikka.texture", &desc)
    }

    pub(super) fn usage(self) -> vk::ImageUsageFlags {
        match self {
            Self::Picture => PICTURE_USAGE,
            Self::Canvas => CANVAS_USAGE,
        }
    }

    pub(super) fn mips(self, image: &GpuImage) -> Mips {
        let chained = mip_levels_for(image.extent()) > 1;
        match self {
            Self::Picture if chained && blits_linearly(image.device(), image.format()) => {
                Mips::Grows
            }
            Self::Picture | Self::Canvas => Mips::Fixed,
        }
    }
}
