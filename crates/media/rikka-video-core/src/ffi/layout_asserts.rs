#![allow(clippy::unnecessary_cast)]

const _: () = assert!(size_of::<crate::ffi::vulkan::AVVulkanDeviceQueueFamily>() == 16);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanDeviceQueueFamily, idx) == 0);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanDeviceQueueFamily, num) == 4);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanDeviceQueueFamily, flags) == 8);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanDeviceQueueFamily, video_caps) == 12);
const _: () = assert!(size_of::<crate::ffi::vulkan::AVVulkanDeviceContext>() == 1360);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanDeviceContext, alloc) == 0);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanDeviceContext, get_proc_addr) == 8);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanDeviceContext, inst) == 16);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanDeviceContext, phys_dev) == 24);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanDeviceContext, act_dev) == 32);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanDeviceContext, device_features) == 40);
const _: () = assert!(
    std::mem::offset_of!(
        crate::ffi::vulkan::AVVulkanDeviceContext,
        enabled_inst_extensions
    ) == 280
);
const _: () = assert!(
    std::mem::offset_of!(
        crate::ffi::vulkan::AVVulkanDeviceContext,
        nb_enabled_inst_extensions
    ) == 288
);
const _: () = assert!(
    std::mem::offset_of!(
        crate::ffi::vulkan::AVVulkanDeviceContext,
        enabled_dev_extensions
    ) == 296
);
const _: () = assert!(
    std::mem::offset_of!(
        crate::ffi::vulkan::AVVulkanDeviceContext,
        nb_enabled_dev_extensions
    ) == 304
);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanDeviceContext, lock_queue) == 312);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanDeviceContext, unlock_queue) == 320);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanDeviceContext, qf) == 328);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanDeviceContext, nb_qf) == 1352);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanDeviceContext, queue_flags) == 1356);
const _: () = assert!(size_of::<crate::ffi::vulkan::AVVulkanFramesContext>() == 144);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanFramesContext, tiling) == 0);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanFramesContext, usage) == 4);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanFramesContext, create_pnext) == 8);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanFramesContext, alloc_pnext) == 16);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanFramesContext, flags) == 80);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanFramesContext, img_flags) == 84);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanFramesContext, format) == 88);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanFramesContext, nb_layers) == 120);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanFramesContext, lock_frame) == 128);
const _: () =
    assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVulkanFramesContext, unlock_frame) == 136);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVkFrame, img) == 0);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVkFrame, tiling) == 64);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVkFrame, mem) == 72);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVkFrame, size) == 136);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVkFrame, flags) == 200);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVkFrame, access) == 208);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVkFrame, layout) == 272);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVkFrame, sem) == 304);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVkFrame, sem_value) == 368);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVkFrame, internal) == 528);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVkFrame, offset) == 432);
const _: () = assert!(std::mem::offset_of!(crate::ffi::vulkan::AVVkFrame, queue_family) == 496);
const _: () = assert!(crate::ffi::vulkan::AV_VK_FRAME_FLAG_NONE as i64 == 1);
const _: () = assert!(crate::ffi::vulkan::AV_VK_FRAME_FLAG_DISABLE_MULTIPLANE as i64 == 4);
