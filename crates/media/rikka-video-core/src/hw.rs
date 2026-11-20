use core::ffi::{c_char, c_void};
use std::ffi::CString;

use ash::vk;
use rikka_gpu::QueueLock;
use rikka_gpu::VideoDeviceHandles;

use crate::error::{Error, Result, check};
use crate::ffi::vulkan::{AVVulkanDeviceContext, AVVulkanDeviceQueueFamily};
use rikka_ffmpeg::BufferRef;
use rikka_ffmpeg::avutil::{self, AVHWDeviceContext};

struct FeatureChain {
    features11: vk::PhysicalDeviceVulkan11Features<'static>,
    features12: vk::PhysicalDeviceVulkan12Features<'static>,
    features13: vk::PhysicalDeviceVulkan13Features<'static>,
    video_maintenance1: vk::PhysicalDeviceVideoMaintenance1FeaturesKHR<'static>,
}

struct HwOpaque {
    lock: QueueLock,
    _instance_extensions: Vec<CString>,
    instance_extension_ptrs: Vec<*const c_char>,
    _device_extensions: Vec<CString>,
    device_extension_ptrs: Vec<*const c_char>,
    _features: Box<FeatureChain>,
    _device: std::sync::Arc<dyn std::any::Any + Send + Sync>,
}

pub struct HwDevice {
    buffer: BufferRef,
    handles: VideoDeviceHandles,
}

impl HwDevice {
    pub fn new(handles: &VideoDeviceHandles) -> Result<Self> {
        let buffer = BufferRef::from_raw(
            unsafe { avutil::av_hwdevice_ctx_alloc(avutil::AV_HWDEVICE_TYPE_VULKAN) },
            "av_hwdevice_ctx_alloc",
        )
        .map_err(|_| {
            Error::Unsupported(
                "FFmpeg was built without Vulkan support (av_hwdevice_ctx_alloc failed)".into(),
            )
        })?;

        let mut features = Box::new(FeatureChain {
            features11: vk::PhysicalDeviceVulkan11Features::default()
                .sampler_ycbcr_conversion(handles.features.sampler_ycbcr_conversion),
            features12: vk::PhysicalDeviceVulkan12Features::default()
                .timeline_semaphore(true)
                .runtime_descriptor_array(true)
                .shader_sampled_image_array_non_uniform_indexing(true)
                .descriptor_binding_sampled_image_update_after_bind(true)
                .descriptor_binding_partially_bound(true)
                .descriptor_binding_update_unused_while_pending(true),
            features13: vk::PhysicalDeviceVulkan13Features::default()
                .dynamic_rendering(true)
                .synchronization2(true),
            video_maintenance1: vk::PhysicalDeviceVideoMaintenance1FeaturesKHR::default()
                .video_maintenance1(handles.features.video_maintenance1),
        });
        features.features11.p_next = (&raw mut features.features12).cast::<c_void>();
        features.features12.p_next = (&raw mut features.features13).cast::<c_void>();
        features.features13.p_next = if handles.features.video_maintenance1 {
            (&raw mut features.video_maintenance1).cast::<c_void>()
        } else {
            std::ptr::null_mut()
        };
        let features11_ptr = (&raw mut features.features11).cast::<c_void>();

        let instance_extensions: Vec<CString> = handles.instance_extensions.clone();
        let instance_extension_ptrs: Vec<*const c_char> =
            instance_extensions.iter().map(|ext| ext.as_ptr()).collect();
        let device_extensions: Vec<CString> = handles.device_extensions.clone();
        let device_extension_ptrs: Vec<*const c_char> =
            device_extensions.iter().map(|ext| ext.as_ptr()).collect();

        let opaque = Box::new(HwOpaque {
            lock: handles.queue_lock.clone(),
            _instance_extensions: instance_extensions,
            instance_extension_ptrs,
            _device_extensions: device_extensions,
            device_extension_ptrs,
            _features: features,
            _device: handles.keepalive(),
        });

        unsafe {
            let device_ctx = (*buffer.as_ptr()).data.cast::<AVHWDeviceContext>();
            let vk_ctx = (*device_ctx).hwctx.cast::<AVVulkanDeviceContext>();

            (*vk_ctx).get_proc_addr = handles.entry.static_fn().get_instance_proc_addr;
            (*vk_ctx).inst = handles.instance;
            (*vk_ctx).phys_dev = handles.physical_device;
            (*vk_ctx).act_dev = handles.device;
            (*vk_ctx).device_features = vk::PhysicalDeviceFeatures2::default();
            (*vk_ctx).device_features.p_next = features11_ptr;
            (*vk_ctx).enabled_inst_extensions = opaque.instance_extension_ptrs.as_ptr();
            (*vk_ctx).nb_enabled_inst_extensions = opaque.instance_extension_ptrs.len() as i32;
            (*vk_ctx).enabled_dev_extensions = opaque.device_extension_ptrs.as_ptr();
            (*vk_ctx).nb_enabled_dev_extensions = opaque.device_extension_ptrs.len() as i32;

            for (slot, family) in (*vk_ctx).qf.iter_mut().zip(handles.queue_families.iter()) {
                *slot = AVVulkanDeviceQueueFamily {
                    idx: family.index as i32,
                    num: 1,
                    flags: family.flags,
                    video_caps: family.video_ops,
                };
            }
            (*vk_ctx).nb_qf = handles.queue_families.len().min(64) as i32;

            (*vk_ctx).lock_queue = Some(lock_queue);
            (*vk_ctx).unlock_queue = Some(unlock_queue);

            (*device_ctx).user_opaque = Box::into_raw(opaque).cast::<c_void>();
            (*device_ctx).free = Some(free_device);
        }

        let result = unsafe { avutil::av_hwdevice_ctx_init(buffer.as_ptr()) };
        check("av_hwdevice_ctx_init", result)?;

        Ok(Self {
            buffer,
            handles: handles.clone(),
        })
    }

    pub fn new_ref(&self) -> Result<BufferRef> {
        Ok(self.buffer.share()?)
    }

    #[must_use]
    pub fn handles(&self) -> &VideoDeviceHandles {
        &self.handles
    }
}

unsafe extern "C" fn lock_queue(ctx: *mut AVHWDeviceContext, _family: u32, _index: u32) {
    let opaque = unsafe { &*(*ctx).user_opaque.cast::<HwOpaque>() };
    opaque.lock.lock();
}

unsafe extern "C" fn unlock_queue(ctx: *mut AVHWDeviceContext, _family: u32, _index: u32) {
    let opaque = unsafe { &*(*ctx).user_opaque.cast::<HwOpaque>() };
    unsafe { opaque.lock.unlock() };
}

unsafe extern "C" fn free_device(ctx: *mut AVHWDeviceContext) {
    drop(unsafe { Box::from_raw((*ctx).user_opaque.cast::<HwOpaque>()) });
}

impl std::fmt::Debug for HwDevice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("HwDevice").finish_non_exhaustive()
    }
}
