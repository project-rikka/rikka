use std::ffi::CString;

use ash::vk::{self, Handle};
use rikka_gpu::{ExternalDevice, ExternalHandles, HostSubmit};

use crate::abi::{AnyThread, Closure, violation};
use crate::ffi;

pub(crate) type Submit = Closure<
    (
        ffi::VulkanSemaphoreFn,
        ffi::VulkanCommandsFn,
        ffi::VulkanSemaphoreFn,
    ),
    AnyThread,
>;

struct HostQueue(Submit);

impl HostSubmit for HostQueue {
    fn submit(&self, batches: &[vk::SubmitInfo2<'_>]) -> Result<(), vk::Result> {
        for batch in batches {
            for wait in parts(
                batch.p_wait_semaphore_infos,
                batch.wait_semaphore_info_count,
            ) {
                self.0.call_wait(
                    wait.semaphore.as_raw(),
                    wait.value,
                    wait.stage_mask.as_raw(),
                );
            }
            for commands in parts(
                batch.p_command_buffer_infos,
                batch.command_buffer_info_count,
            ) {
                self.0.call_execute(commands.command_buffer.as_raw());
            }
            for signal in parts(
                batch.p_signal_semaphore_infos,
                batch.signal_semaphore_info_count,
            ) {
                self.0.call_signal(
                    signal.semaphore.as_raw(),
                    signal.value,
                    signal.stage_mask.as_raw(),
                );
            }
        }
        Ok(())
    }
}

fn parts<'a, T>(first: *const T, count: u32) -> &'a [T] {
    match count {
        0 => &[],
        count => unsafe { std::slice::from_raw_parts(first, count as usize) },
    }
}

#[derive(Debug)]
pub(crate) struct VulkanDevice {
    device: Option<ExternalDevice>,
}

pub(crate) struct Handles {
    pub(crate) get_instance_proc_addr: u64,
    pub(crate) instance: u64,
    pub(crate) api_version: u32,
    pub(crate) physical_device: u64,
    pub(crate) device: u64,
    pub(crate) queue: u64,
    pub(crate) queue_family: u32,
}

impl VulkanDevice {
    pub(crate) fn new(handles: Handles, submit: Submit) -> Self {
        let Handles {
            get_instance_proc_addr,
            instance,
            api_version,
            physical_device,
            device,
            queue,
            queue_family,
        } = handles;
        let handles = ExternalHandles {
            get_instance_proc_addr: function(get_instance_proc_addr),
            instance: vk::Instance::from_raw(present(instance, "instance")),
            api_version,
            physical_device: vk::PhysicalDevice::from_raw(present(
                physical_device,
                "physical_device",
            )),
            device: vk::Device::from_raw(present(device, "device")),
            queue: vk::Queue::from_raw(present(queue, "queue")),
            queue_family,
        };
        Self {
            device: Some(unsafe { ExternalDevice::new(handles, HostQueue(submit)) }),
        }
    }

    pub(crate) fn add_instance_extension(&mut self, name: &str) {
        let name = extension(name);
        self.edit(|device| device.instance_extension(&name));
    }

    pub(crate) fn add_device_extension(&mut self, name: &str) {
        let name = extension(name);
        self.edit(|device| device.device_extension(&name));
    }

    pub(crate) fn set_features(&mut self, features: u64) {
        let features = present(features, "features") as usize as *const vk::PhysicalDeviceFeatures2;
        let features = unsafe { &*features };
        if features.s_type != vk::StructureType::PHYSICAL_DEVICE_FEATURES_2 {
            violation("`features` is not a VkPhysicalDeviceFeatures2");
        }
        self.edit(|device| unsafe { device.enabled_features(features) });
    }

    pub(crate) fn into_device(self) -> ExternalDevice {
        self.device.expect("a Vulkan device holds its description")
    }

    fn edit(&mut self, change: impl FnOnce(ExternalDevice) -> ExternalDevice) {
        let device = self
            .device
            .take()
            .expect("a Vulkan device holds its description");
        self.device = Some(change(device));
    }
}

fn present(handle: u64, param: &str) -> u64 {
    if handle == 0 {
        violation(format!("`{param}` is NULL"));
    }
    handle
}

fn function(address: u64) -> vk::PFN_vkGetInstanceProcAddr {
    let address = present(address, "get_instance_proc_addr") as usize;
    unsafe { std::mem::transmute::<usize, vk::PFN_vkGetInstanceProcAddr>(address) }
}

fn extension(name: &str) -> CString {
    if name.is_empty() {
        violation("the extension name is empty");
    }
    CString::new(name).unwrap_or_else(|_| violation(format!("`{name}` holds a NUL")))
}
