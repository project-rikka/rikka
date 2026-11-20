use ash::vk;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    R8,
    R16,
    R32Float,
    Rgba8,
    Rgba16,
    Rgba16Float,
    Rgba32Float,
}

impl ImageFormat {
    pub(crate) const ALL: [ImageFormat; 7] = [
        ImageFormat::R8,
        ImageFormat::R16,
        ImageFormat::R32Float,
        ImageFormat::Rgba8,
        ImageFormat::Rgba16,
        ImageFormat::Rgba16Float,
        ImageFormat::Rgba32Float,
    ];

    #[must_use]
    pub const fn bytes_per_pixel(self) -> u32 {
        match self {
            ImageFormat::R8 => 1,
            ImageFormat::R16 => 2,
            ImageFormat::R32Float | ImageFormat::Rgba8 => 4,
            ImageFormat::Rgba16 | ImageFormat::Rgba16Float => 8,
            ImageFormat::Rgba32Float => 16,
        }
    }

    pub(crate) const fn vk(self) -> vk::Format {
        match self {
            ImageFormat::R8 => vk::Format::R8_UNORM,
            ImageFormat::R16 => vk::Format::R16_UNORM,
            ImageFormat::R32Float => vk::Format::R32_SFLOAT,
            ImageFormat::Rgba8 => vk::Format::R8G8B8A8_UNORM,
            ImageFormat::Rgba16 => vk::Format::R16G16B16A16_UNORM,
            ImageFormat::Rgba16Float => vk::Format::R16G16B16A16_SFLOAT,
            ImageFormat::Rgba32Float => vk::Format::R32G32B32A32_SFLOAT,
        }
    }
}
