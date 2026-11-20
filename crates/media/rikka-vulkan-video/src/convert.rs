use ash::vk;
use rikka_gpu::{VideoFrameImage, VideoPlane, VideoTarget};

pub(crate) struct Target {
    pub(crate) image: vk::Image,
    pub(crate) surface: (u32, u32),
    pub(crate) signal: (vk::Semaphore, u64),
}

impl Target {
    pub(crate) fn written(&self) -> VideoTarget {
        VideoTarget {
            extent: [self.surface.0, self.surface.1],
            planes: vec![
                VideoPlane {
                    image: self.image,
                    aspect: vk::ImageAspectFlags::PLANE_0,
                    view_format: vk::Format::R8_UNORM,
                },
                VideoPlane {
                    image: self.image,
                    aspect: vk::ImageAspectFlags::PLANE_1,
                    view_format: vk::Format::R8G8_UNORM,
                },
            ],
            images: vec![VideoFrameImage {
                image: self.image,
                layout: vk::ImageLayout::UNDEFINED,
                source_queue_family: None,
            }],
            waits: Vec::new(),
            signals: vec![self.signal],
            keepalive: Box::new(()),
        }
    }
}
