use ash::vk;
use rikka_colorimetry::BlendSpace;
use rikka_types::Transform;

use crate::fx::FX_FORMAT;
use crate::scene::Viewer;
use rikka_gpu::core::image::GpuImage;
use rikka_gpu::reclaim::{Deferred, TargetLease};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TargetFormat {
    #[default]
    Rgba8,
    Rgb10a2,
}

impl TargetFormat {
    pub(crate) fn vk_format(self) -> vk::Format {
        match self {
            Self::Rgba8 => FX_FORMAT,
            Self::Rgb10a2 => vk::Format::A2B10G10R10_UNORM_PACK32,
        }
    }
}

pub struct OffscreenTarget {
    pub(crate) image: Deferred<GpuImage>,
    pub(crate) lease: TargetLease,
    pub(crate) transform: Transform,
    pub(crate) viewer: Viewer,
    pub(crate) space: BlendSpace,
}

impl OffscreenTarget {
    pub fn set_transform(&mut self, transform: Transform) {
        self.transform = transform;
    }

    pub fn transform(&self) -> Transform {
        self.transform
    }

    pub fn set_viewer(&mut self, viewer: Viewer) {
        self.viewer = viewer;
    }

    pub fn set_blend_space(&mut self, space: BlendSpace) {
        self.space = space;
    }

    pub fn width(&self) -> u32 {
        self.image.extent().width
    }

    pub fn height(&self) -> u32 {
        self.image.extent().height
    }
}

impl std::fmt::Debug for OffscreenTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OffscreenTarget")
            .field("width", &self.width())
            .field("height", &self.height())
            .finish_non_exhaustive()
    }
}
