use ash::vk;

use crate::core::image::Texels;

#[derive(Default)]
pub(super) struct Writes {
    pub(super) fresh: bool,
    pub(super) clears: bool,
    pub(super) placed: Vec<(Texels, usize)>,
    pub(super) copies: Vec<(usize, CopySource)>,
    pub(super) texels: Option<Texels>,
}

pub(super) enum CopySource {
    Staged(vk::Buffer, vk::BufferImageCopy),
    Image(vk::Image, vk::ImageLayout, vk::ImageCopy),
}

impl CopySource {
    pub(super) fn same_buffer(&self, other: &CopySource) -> bool {
        matches!(
            (self, other),
            (CopySource::Staged(a, _), CopySource::Staged(b, _)) if a == b
        )
    }
}
