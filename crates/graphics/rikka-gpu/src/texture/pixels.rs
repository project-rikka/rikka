use half::f16;

use crate::texture::TextureFormat;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pixels {
    pub(crate) extent: [u32; 2],
    pub(crate) format: TextureFormat,
    pub(crate) data: Vec<u8>,
}

impl Pixels {
    #[must_use]
    pub fn new(width: u32, height: u32, format: TextureFormat, data: impl Into<Vec<u8>>) -> Self {
        Self {
            extent: [width, height],
            format,
            data: data.into(),
        }
    }

    #[must_use]
    pub fn rgba8(width: u32, height: u32, data: impl Into<Vec<u8>>) -> Self {
        Self::new(width, height, TextureFormat::Rgba8, data)
    }

    #[must_use]
    pub fn premultiplied(width: u32, height: u32, data: impl Into<Vec<u8>>) -> Self {
        Self::new(width, height, TextureFormat::Rgba8Premultiplied, data)
    }

    #[must_use]
    pub fn srgb(width: u32, height: u32, data: impl Into<Vec<u8>>) -> Self {
        Self::new(width, height, TextureFormat::Rgba8Srgb, data)
    }

    #[must_use]
    pub fn rgba16f(width: u32, height: u32, samples: &[f32]) -> Self {
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|&sample| f16::from_f32(sample).to_le_bytes())
            .collect();
        Self::new(width, height, TextureFormat::Rgba16Float, bytes)
    }

    #[must_use]
    pub fn rgba32f(width: u32, height: u32, samples: &[f32]) -> Self {
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        Self::new(width, height, TextureFormat::Rgba32Float, bytes)
    }

    #[must_use]
    pub fn extent(&self) -> [u32; 2] {
        self.extent
    }
}
