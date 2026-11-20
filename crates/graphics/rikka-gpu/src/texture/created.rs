use std::sync::Arc;

use ash::vk;

use crate::core::device::RenderDevice;
use crate::core::image::{GpuImage, ImageDesc};
#[cfg(unix)]
use crate::drm::{DmabufSpec, TextureLayout, Ycbcr, import_color, import_planar, texture_format};
use crate::error::{Error, Result};

use super::OUTPUT_FORMAT;
use super::book::{Listing, Mips};
use super::packed::Packed;
use super::pending_upload::Fill;
use super::pixels::Pixels;
use super::prepare::prepared;
use super::source::TextureSource;
use super::usage::Use;

pub struct Created {
    pub(super) image: GpuImage,
    pub(super) source: TextureSource,
    usage: Use,
    pub(super) fill: Option<Fill>,
    texel_bytes: Option<u32>,
    pub(super) components: vk::ComponentMapping,
    pub(super) shared_by: Option<[u32; 2]>,
}

impl Created {
    fn new(image: GpuImage, source: TextureSource, usage: Use, fill: Option<Fill>) -> Self {
        Self {
            image,
            source,
            usage,
            fill,
            texel_bytes: None,
            components: vk::ComponentMapping::default(),
            shared_by: None,
        }
    }

    pub(crate) fn picture(device: &Arc<RenderDevice>, picture: Pixels) -> Result<Created> {
        let [width, height] = picture.extent;
        let format = picture.format;
        let pixels = prepared(picture)?;
        let usage = Use::Picture;
        let image = usage.image(device, vk::Extent2D { width, height }, format.vk_format())?;
        Ok(Created {
            texel_bytes: format.texel_bytes(),
            ..Created::new(
                image,
                TextureSource::Memory,
                usage,
                Some(Fill::Pixels(pixels)),
            )
        })
    }

    pub(crate) fn drm(
        device: &Arc<RenderDevice>,
        fourcc: u32,
        width: u32,
        height: u32,
    ) -> Result<Created> {
        let packed = Packed::from_fourcc(fourcc)
            .filter(|packed| packed.sampled_on(device))
            .ok_or_else(|| {
                Error::Unsupported(format!(
                    "DRM format {fourcc:#010x} is not supported for row uploads"
                ))
            })?;
        let shared_by = device
            .transfer_family()
            .map(|transfer| [device.queue_family(), transfer]);
        let usage = Use::Picture;
        let desc = ImageDesc::color(vk::Extent2D { width, height }, packed.format, usage.usage())
            .components(packed.components)
            .shared_by(shared_by);
        let image = usage.image_as(device, desc)?;
        Ok(Created {
            texel_bytes: Some(packed.texel_bytes),
            components: packed.components,
            shared_by,
            ..Created::new(image, TextureSource::Memory, usage, Some(Fill::Blank))
        })
    }

    pub(crate) fn compute_target(
        device: &Arc<RenderDevice>,
        width: u32,
        height: u32,
    ) -> Result<Created> {
        let usage = Use::Picture;
        let image = usage.image(device, vk::Extent2D { width, height }, OUTPUT_FORMAT)?;
        Ok(Created {
            texel_bytes: Some(4),
            ..Created::new(image, TextureSource::Memory, usage, Some(Fill::Blank))
        })
    }

    pub(crate) fn blank(device: &Arc<RenderDevice>, width: u32, height: u32) -> Result<Created> {
        let usage = Use::Canvas;
        let image = usage.image(device, vk::Extent2D { width, height }, OUTPUT_FORMAT)?;
        Ok(Created::new(
            image,
            TextureSource::Memory,
            usage,
            Some(Fill::Blank),
        ))
    }

    #[cfg(unix)]
    pub(crate) fn dmabuf(
        device: &Arc<RenderDevice>,
        spec: &DmabufSpec<'_>,
        ycbcr: Ycbcr,
    ) -> Result<Created> {
        let format = texture_format(spec.fourcc).ok_or_else(|| {
            Error::Unsupported(format!(
                "DRM format {:#010x} is not supported for sampling",
                spec.fourcc
            ))
        })?;
        let (image, source) = match format.layout {
            TextureLayout::Color { .. } => (
                import_color(device.clone(), spec, format)?,
                TextureSource::Dmabuf,
            ),
            TextureLayout::Planar(_) => {
                let planes = import_planar(device.clone(), spec, format, ycbcr)?;
                (
                    Use::Canvas.image(device, planes.extent(), planes.converted_format())?,
                    TextureSource::Planar(planes),
                )
            }
        };
        Ok(Created::new(image, source, Use::Canvas, None))
    }

    pub(crate) fn listing(&self, slot: u32) -> Listing {
        let extent = self.image.extent();
        Listing {
            slot,
            extent: [extent.width, extent.height],
            kind: self.source.kind(self.usage),
            mips: match self.source.is_imported() {
                true => Mips::Fixed,
                false => self.usage.mips(&self.image),
            },
            texel_bytes: self.texel_bytes,
        }
    }
}

impl std::fmt::Debug for Created {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Created").finish_non_exhaustive()
    }
}
