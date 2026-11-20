use std::any::Any;
use std::sync::Arc;

use ash::vk;
use rikka_gpu::{Gpu, VideoDeviceHandles};

use crate::error::{Error, Result};

pub(crate) struct VideoDevice {
    pub(crate) device: ash::Device,
    pub(crate) physical: vk::PhysicalDevice,
    pub(crate) queue_instance: ash::khr::video_queue::InstanceFn,
    pub(crate) queue_fns: ash::khr::video_queue::DeviceFn,
    pub(crate) encode_fns: ash::khr::video_encode_queue::DeviceFn,
    pub(crate) graphics_family: u32,
    pub(crate) encode_family: u32,
    pub(crate) encode_ops: vk::VideoCodecOperationFlagsKHR,
    pub(crate) queue: vk::Queue,
    pub(crate) memory: vk::PhysicalDeviceMemoryProperties,
    pub(crate) name: String,
    _keepalive: Arc<dyn Any + Send + Sync>,
}

impl VideoDevice {
    pub(crate) fn open(gpu: &Gpu) -> Result<Arc<VideoDevice>> {
        let handles = gpu.video_device().ok_or(Error::NoVideoQueue)?;
        VideoDevice::from_handles(handles).map(Arc::new)
    }

    fn from_handles(handles: VideoDeviceHandles) -> Result<VideoDevice> {
        let family = handles
            .queue_families
            .iter()
            .find(|family| family.flags.contains(vk::QueueFlags::VIDEO_ENCODE_KHR))
            .copied()
            .ok_or(Error::NoVideoQueue)?;
        let instance = unsafe { ash::Instance::load(handles.entry.static_fn(), handles.instance) };
        let device = unsafe { ash::Device::load(instance.fp_v1_0(), handles.device) };
        let load_instance = |name: &std::ffi::CStr| unsafe {
            std::mem::transmute::<vk::PFN_vkVoidFunction, *const std::ffi::c_void>(
                handles
                    .entry
                    .get_instance_proc_addr(handles.instance, name.as_ptr()),
            )
        };
        let load_device = |name: &std::ffi::CStr| unsafe {
            std::mem::transmute::<vk::PFN_vkVoidFunction, *const std::ffi::c_void>(
                instance.get_device_proc_addr(handles.device, name.as_ptr()),
            )
        };
        let queue_instance = ash::khr::video_queue::InstanceFn::load(load_instance);
        let queue_fns = ash::khr::video_queue::DeviceFn::load(load_device);
        let encode_fns = ash::khr::video_encode_queue::DeviceFn::load(load_device);
        let queue = unsafe { device.get_device_queue(family.index, 0) };
        let memory =
            unsafe { instance.get_physical_device_memory_properties(handles.physical_device) };
        let properties =
            unsafe { instance.get_physical_device_properties(handles.physical_device) };
        let name = properties
            .device_name_as_c_str()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(VideoDevice {
            device,
            physical: handles.physical_device,
            queue_instance,
            queue_fns,
            encode_fns,
            graphics_family: handles.graphics_queue_family,
            encode_family: family.index,
            encode_ops: family.video_ops,
            queue,
            memory,
            name,
            _keepalive: handles.keepalive(),
        })
    }

    pub(crate) fn families(&self) -> Vec<u32> {
        let mut families = vec![self.graphics_family];
        if self.encode_family != self.graphics_family {
            families.push(self.encode_family);
        }
        families
    }

    pub(crate) fn memory_type(
        &self,
        bits: u32,
        wanted: vk::MemoryPropertyFlags,
        fallback: vk::MemoryPropertyFlags,
    ) -> Result<u32> {
        let types = &self.memory.memory_types[..self.memory.memory_type_count as usize];
        let find = |flags: vk::MemoryPropertyFlags| {
            types
                .iter()
                .zip(0_u32..)
                .find(|(kind, index)| {
                    bits & (1 << index) != 0 && kind.property_flags.contains(flags)
                })
                .map(|(_, index)| index)
        };
        find(wanted)
            .or_else(|| find(fallback))
            .ok_or(Error::NoMemoryType)
    }

    pub(crate) fn allocate(
        &self,
        requirements: vk::MemoryRequirements,
        wanted: vk::MemoryPropertyFlags,
        fallback: vk::MemoryPropertyFlags,
    ) -> Result<vk::DeviceMemory> {
        let index = self.memory_type(requirements.memory_type_bits, wanted, fallback)?;
        let info = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(index);
        Ok(unsafe { self.device.allocate_memory(&info, None) }?)
    }
}

impl std::fmt::Debug for VideoDevice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VideoDevice")
            .field("name", &self.name)
            .field("encode_family", &self.encode_family)
            .finish_non_exhaustive()
    }
}
