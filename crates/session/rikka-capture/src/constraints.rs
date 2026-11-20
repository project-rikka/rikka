use rikka_gpu::Gpu;

use crate::format::PixelFormat;

const SAMPLED_ORDER: [PixelFormat; 4] = [
    PixelFormat::Xrgb8888,
    PixelFormat::Argb8888,
    PixelFormat::Xbgr8888,
    PixelFormat::Abgr8888,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DmabufFormat {
    pub format: PixelFormat,
    pub modifiers: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DmabufFormats {
    pub device: u64,
    pub formats: Vec<DmabufFormat>,
}

impl DmabufFormats {
    #[must_use]
    pub fn modifiers(&self, format: PixelFormat) -> &[u64] {
        self.formats
            .iter()
            .find(|candidate| candidate.format == format)
            .map_or(&[], |candidate| candidate.modifiers.as_slice())
    }

    #[must_use]
    pub fn sampled_by(&self, gpu: &Gpu) -> Option<(PixelFormat, Vec<u64>)> {
        let textures = gpu.dmabuf_texture_formats();
        SAMPLED_ORDER.into_iter().find_map(|format| {
            let modifiers: Vec<u64> = self
                .modifiers(format)
                .iter()
                .copied()
                .filter(|modifier| {
                    textures.iter().any(|texture| {
                        texture.fourcc == format.fourcc() && texture.modifier == *modifier
                    })
                })
                .collect();
            (!modifiers.is_empty()).then_some((format, modifiers))
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Constraints {
    pub width: u32,
    pub height: u32,
    pub shm: Vec<PixelFormat>,
    pub dmabuf: Option<DmabufFormats>,
    pub generation: u64,
}

impl Constraints {
    #[must_use]
    pub fn preferred_shm(&self, order: &[PixelFormat]) -> Option<PixelFormat> {
        order
            .iter()
            .copied()
            .find(|format| self.shm.contains(format))
    }

    #[must_use]
    pub fn best_shm(&self) -> Option<PixelFormat> {
        self.preferred_shm(&PixelFormat::ALL)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}
