#![allow(non_camel_case_types, non_snake_case, dead_code)]

use core::ffi::{c_char, c_int, c_void};

use ash::vk;

use rikka_ffmpeg::avutil::{AVHWDeviceContext, AVHWFramesContext};

pub const AV_VK_MAX_QUEUE_FAMILIES: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct AVVulkanDeviceQueueFamily {
    pub idx: c_int,
    pub num: c_int,
    pub flags: vk::QueueFlags,
    pub video_caps: vk::VideoCodecOperationFlagsKHR,
}

#[repr(C)]
pub struct AVVulkanDeviceContext {
    pub alloc: *const c_void,
    pub get_proc_addr: vk::PFN_vkGetInstanceProcAddr,
    pub inst: vk::Instance,
    pub phys_dev: vk::PhysicalDevice,
    pub act_dev: vk::Device,
    pub device_features: vk::PhysicalDeviceFeatures2<'static>,
    pub enabled_inst_extensions: *const *const c_char,
    pub nb_enabled_inst_extensions: c_int,
    pub enabled_dev_extensions: *const *const c_char,
    pub nb_enabled_dev_extensions: c_int,
    pub lock_queue:
        Option<unsafe extern "C" fn(ctx: *mut AVHWDeviceContext, queue_family: u32, index: u32)>,
    pub unlock_queue:
        Option<unsafe extern "C" fn(ctx: *mut AVHWDeviceContext, queue_family: u32, index: u32)>,
    pub qf: [AVVulkanDeviceQueueFamily; AV_VK_MAX_QUEUE_FAMILIES],
    pub nb_qf: c_int,
    pub queue_flags: vk::DeviceQueueCreateFlags,
}

pub type AVVkFrameFlags = c_int;
pub const AV_VK_FRAME_FLAG_NONE: AVVkFrameFlags = 1 << 0;
pub const AV_VK_FRAME_FLAG_DISABLE_MULTIPLANE: AVVkFrameFlags = 1 << 2;

#[repr(C)]
pub struct AVVulkanFramesContext {
    pub tiling: vk::ImageTiling,
    pub usage: vk::ImageUsageFlags,
    pub create_pnext: *mut c_void,
    pub alloc_pnext: [*mut c_void; 8],
    pub flags: AVVkFrameFlags,
    pub img_flags: vk::ImageCreateFlags,
    pub format: [vk::Format; 8],
    pub nb_layers: c_int,
    pub lock_frame: Option<unsafe extern "C" fn(fc: *mut AVHWFramesContext, vkf: *mut AVVkFrame)>,
    pub unlock_frame: Option<unsafe extern "C" fn(fc: *mut AVHWFramesContext, vkf: *mut AVVkFrame)>,
}

#[repr(C)]
pub struct AVVkFrame {
    pub img: [vk::Image; 8],
    pub tiling: vk::ImageTiling,
    pub mem: [vk::DeviceMemory; 8],
    pub size: [usize; 8],
    pub flags: vk::MemoryPropertyFlags,
    pub access: [vk::AccessFlags2; 8],
    pub layout: [vk::ImageLayout; 8],
    pub sem: [vk::Semaphore; 8],
    pub sem_value: [u64; 8],
    pub offset: [isize; 8],
    pub queue_family: [u32; 8],
    pub internal: *mut c_void,
}

impl std::fmt::Debug for AVVulkanDeviceContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AVVulkanDeviceContext")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for AVVulkanFramesContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AVVulkanFramesContext")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for AVVkFrame {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("AVVkFrame").finish_non_exhaustive()
    }
}
