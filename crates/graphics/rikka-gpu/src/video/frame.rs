use ash::vk;

use crate::core::image::{ImageAccess, QueueTransfer, color_subresource_range, image_barrier};
use crate::video::lease::VideoLease;

pub(crate) const WAIT_STAGE: vk::PipelineStageFlags2 = vk::PipelineStageFlags2::FRAGMENT_SHADER;

#[derive(Clone, Copy, Debug)]
pub struct VideoPlane {
    pub image: vk::Image,
    pub aspect: vk::ImageAspectFlags,
    pub view_format: vk::Format,
}

#[derive(Clone, Copy, Debug)]
pub struct VideoFrameImage {
    pub image: vk::Image,
    pub layout: vk::ImageLayout,
    pub source_queue_family: Option<u32>,
}

impl VideoFrameImage {
    pub(crate) fn handover(&self, family: u32) -> Option<vk::ImageMemoryBarrier2<'static>> {
        let next = ImageAccess::FRAGMENT_SAMPLED;
        if self.layout == next.layout && self.source_queue_family.is_none() {
            return None;
        }
        let queues = match self.source_queue_family {
            Some(source) => QueueTransfer {
                src: source,
                dst: family,
            },
            None => QueueTransfer::NONE,
        };
        let current = ImageAccess {
            stage: WAIT_STAGE,
            access: vk::AccessFlags2::NONE,
            layout: self.layout,
        };
        Some(image_barrier(
            self.image,
            current,
            next,
            queues,
            color_subresource_range(),
        ))
    }
}

pub struct ExternalVideoFrame {
    pub planes: Vec<VideoPlane>,
    pub chroma_packed: bool,
    pub uv_scale: [f32; 2],
    pub color_matrix: [[f32; 4]; 3],
    pub lease: VideoLease,
}

impl std::fmt::Debug for ExternalVideoFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalVideoFrame")
            .field("planes", &self.planes)
            .field("chroma_packed", &self.chroma_packed)
            .field("uv_scale", &self.uv_scale)
            .finish_non_exhaustive()
    }
}
