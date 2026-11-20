use ash::vk;

use crate::core::image::GpuImage;

use super::source::TextureSource;

pub(super) struct TextureEntry {
    pub(super) image: GpuImage,
    pub(super) tex: u32,
    pub(super) source: TextureSource,
    pub(super) committed: bool,
    pub(super) components: vk::ComponentMapping,
    pub(super) shared_by: Option<[u32; 2]>,
    pub(super) arrival: u64,
}
