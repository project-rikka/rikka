use ash::vk;

use super::OUTPUT_FORMAT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureFormat {
    Rgba8Premultiplied,
    Rgba8,
    Rgba8Srgb,
    Rgba16Float,
    Rgba32Float,
}

impl TextureFormat {
    pub(super) fn vk_format(self) -> vk::Format {
        match self {
            Self::Rgba8Premultiplied | Self::Rgba8 => OUTPUT_FORMAT,
            Self::Rgba8Srgb => vk::Format::R8G8B8A8_SRGB,
            Self::Rgba16Float => vk::Format::R16G16B16A16_SFLOAT,
            Self::Rgba32Float => vk::Format::R32G32B32A32_SFLOAT,
        }
    }

    pub(super) fn premultiplies(self) -> bool {
        self == Self::Rgba8
    }

    #[must_use]
    pub fn bytes_per_texel(self) -> u32 {
        match self {
            Self::Rgba8Premultiplied | Self::Rgba8 | Self::Rgba8Srgb => 4,
            Self::Rgba16Float => 8,
            Self::Rgba32Float => 16,
        }
    }

    pub(super) fn texel_bytes(self) -> Option<u32> {
        match self {
            Self::Rgba8 => None,
            Self::Rgba8Premultiplied | Self::Rgba8Srgb | Self::Rgba16Float | Self::Rgba32Float => {
                Some(self.bytes_per_texel())
            }
        }
    }
}
