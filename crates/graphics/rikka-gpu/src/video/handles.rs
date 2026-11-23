use std::any::Any;
use std::ffi::CString;
use std::sync::Arc;

use ash::vk;

use crate::core::device::RenderDevice;
use crate::core::queue_lock::QueueLockShared;
use crate::video::lease::{LentFrame, VideoLease};
use crate::video::releaser::VideoReleaser;

#[derive(Clone)]
pub struct QueueLock(Arc<QueueLockShared>);

impl QueueLock {
    pub(crate) fn new(shared: Arc<QueueLockShared>) -> Self {
        Self(shared)
    }

    pub fn lock(&self) {
        self.0.lock();
    }

    pub unsafe fn unlock(&self) {
        unsafe { self.0.unlock() };
    }
}

impl Default for QueueLock {
    fn default() -> Self {
        Self::new(Arc::new(QueueLockShared::new()))
    }
}

impl std::fmt::Debug for QueueLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueueLock").finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VideoQueueFamily {
    pub index: u32,
    pub flags: vk::QueueFlags,
    pub video_ops: vk::VideoCodecOperationFlagsKHR,
}

#[derive(Clone, Copy, Debug)]
pub struct VideoDeviceFeatures {
    pub sampler_ycbcr_conversion: bool,
    pub video_maintenance1: bool,
}

#[derive(Clone)]
pub struct VideoDeviceHandles {
    pub entry: ash::Entry,
    pub instance: vk::Instance,
    pub physical_device: vk::PhysicalDevice,
    pub device: vk::Device,
    pub graphics_queue_family: u32,
    pub queue_families: Vec<VideoQueueFamily>,
    pub instance_extensions: Vec<CString>,
    pub device_extensions: Vec<CString>,
    pub features: VideoDeviceFeatures,
    pub queue_lock: QueueLock,
    releaser: Arc<VideoReleaser>,
}

impl VideoDeviceHandles {
    #[must_use]
    pub fn keepalive(&self) -> Arc<dyn Any + Send + Sync> {
        self.releaser.clone()
    }

    #[must_use]
    pub fn lease(&self, frame: impl LentFrame) -> VideoLease {
        VideoLease::new(self.releaser.clone(), Box::new(frame))
    }
}

impl std::fmt::Debug for VideoDeviceHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoDeviceHandles")
            .field("graphics_queue_family", &self.graphics_queue_family)
            .field("queue_families", &self.queue_families)
            .field("features", &self.features)
            .finish_non_exhaustive()
    }
}

pub fn video_handles(device: &Arc<RenderDevice>) -> Option<VideoDeviceHandles> {
    let support = device.video()?;
    let instance = device.instance();

    let family_properties = unsafe {
        instance
            .handle()
            .get_physical_device_queue_family_properties(device.physical())
    };
    let graphics_family = device.queue_family();
    let graphics_flags = family_properties
        .get(graphics_family as usize)
        .map(|family| family.queue_flags)
        .unwrap_or(vk::QueueFlags::GRAPHICS);

    let mut queue_families = vec![VideoQueueFamily {
        index: graphics_family,
        flags: graphics_flags,
        video_ops: vk::VideoCodecOperationFlagsKHR::NONE,
    }];
    queue_families.extend(support.families.iter().map(|family| VideoQueueFamily {
        index: family.index,
        flags: family.flags,
        video_ops: family.video_ops,
    }));

    Some(VideoDeviceHandles {
        entry: instance.entry().clone(),
        instance: instance.handle().handle(),
        physical_device: device.physical(),
        device: device.handle().handle(),
        graphics_queue_family: graphics_family,
        queue_families,
        instance_extensions: instance.extensions().to_vec(),
        device_extensions: device.extensions().to_vec(),
        features: VideoDeviceFeatures {
            sampler_ycbcr_conversion: true,
            video_maintenance1: support.video_maintenance1,
        },
        queue_lock: QueueLock::new(device.graphics_queue().lock().clone()),
        releaser: Arc::new(VideoReleaser::new(device.clone())),
    })
}
