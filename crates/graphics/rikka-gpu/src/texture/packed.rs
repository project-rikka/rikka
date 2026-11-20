use ash::vk;

use crate::core::device::RenderDevice;

const SAMPLED_COPIES: vk::FormatFeatureFlags = vk::FormatFeatureFlags::from_raw(
    vk::FormatFeatureFlags::SAMPLED_IMAGE.as_raw()
        | vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR.as_raw()
        | vk::FormatFeatureFlags::TRANSFER_DST.as_raw(),
);

pub(crate) const fn fourcc(code: &[u8; 4]) -> u32 {
    (code[0] as u32) | ((code[1] as u32) << 8) | ((code[2] as u32) << 16) | ((code[3] as u32) << 24)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ColorFormat {
    pub fourcc: u32,
    pub image: vk::Format,
    pub opaque: bool,
}

const fn color(code: &[u8; 4], image: vk::Format, opaque: bool) -> ColorFormat {
    ColorFormat {
        fourcc: fourcc(code),
        image,
        opaque,
    }
}

pub(crate) const COLOR_FORMATS: [ColorFormat; 11] = [
    color(b"XR24", vk::Format::B8G8R8A8_UNORM, true),
    color(b"AR24", vk::Format::B8G8R8A8_UNORM, false),
    color(b"XB24", vk::Format::R8G8B8A8_UNORM, true),
    color(b"AB24", vk::Format::R8G8B8A8_UNORM, false),
    color(b"XR30", vk::Format::A2R10G10B10_UNORM_PACK32, true),
    color(b"AR30", vk::Format::A2R10G10B10_UNORM_PACK32, false),
    color(b"XB30", vk::Format::A2B10G10R10_UNORM_PACK32, true),
    color(b"AB30", vk::Format::A2B10G10R10_UNORM_PACK32, false),
    color(b"XB4H", vk::Format::R16G16B16A16_SFLOAT, true),
    color(b"AB4H", vk::Format::R16G16B16A16_SFLOAT, false),
    color(b"RG16", vk::Format::R5G6B5_UNORM_PACK16, true),
];

pub(crate) fn color_format(code: u32) -> Option<ColorFormat> {
    COLOR_FORMATS
        .iter()
        .find(|format| format.fourcc == code)
        .copied()
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Packed {
    pub format: vk::Format,
    pub components: vk::ComponentMapping,
    pub texel_bytes: u32,
}

impl Packed {
    pub(crate) fn from_fourcc(fourcc: u32) -> Option<Self> {
        let found = color_format(fourcc)?;
        let texel_bytes = match found.image {
            vk::Format::R5G6B5_UNORM_PACK16 => 2,
            vk::Format::R16G16B16A16_SFLOAT => 8,
            _ => 4,
        };
        let components = match found.opaque {
            true => vk::ComponentMapping::default().a(vk::ComponentSwizzle::ONE),
            false => vk::ComponentMapping::default(),
        };
        Some(Self {
            format: found.image,
            components,
            texel_bytes,
        })
    }

    pub(crate) fn sampled_on(&self, device: &RenderDevice) -> bool {
        let properties = unsafe {
            device
                .instance()
                .handle()
                .get_physical_device_format_properties(device.physical(), self.format)
        };
        properties.optimal_tiling_features.contains(SAMPLED_COPIES)
    }
}
