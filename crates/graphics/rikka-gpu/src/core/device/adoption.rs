use std::ffi::CStr;

use ash::{ext, vk};

use crate::core::caps::FeatureRequest;
#[cfg(unix)]
use crate::core::dmabuf_support;
use crate::core::instance::{Ownership, VulkanInstance};
use crate::core::memory::MemoryFeatures;
use crate::core::queue_lock::LockedQueue;
use crate::error::{Error, Result};
use crate::external::ExternalDevice;

use super::creation::Finish;
use super::request::DeviceRequest;
#[cfg(unix)]
use super::selection::device_extensions;
use super::{Interop, RenderDevice};

impl RenderDevice {
    pub fn adopt(external: ExternalDevice, request: DeviceRequest) -> Result<Self> {
        refuse_beside_external(request)?;
        let ExternalDevice {
            handles,
            instance_extensions,
            device_extensions: extensions,
            features,
            submit,
        } = external;
        let instance = unsafe {
            VulkanInstance::adopt(
                handles.get_instance_proc_addr,
                handles.instance,
                handles.api_version,
                instance_extensions,
            )?
        };
        let physical = handles.physical_device;
        let properties = unsafe { instance.handle().get_physical_device_properties(physical) };
        let name = properties
            .device_name_as_c_str()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "<unknown>".to_owned());
        if properties.api_version < vk::API_VERSION_1_3 {
            return Err(Error::Unsupported(format!(
                "{name}: Vulkan 1.3 not supported"
            )));
        }
        let wants = FeatureRequest {
            ycbcr: features.features11.sampler_ycbcr_conversion == vk::TRUE,
            three_d: request.three_d,
            compute: request.compute,
            anisotropy: features.features10.sampler_anisotropy == vk::TRUE,
            float16: features.features12.shader_float16 == vk::TRUE,
            int16: features.features10.shader_int16 == vk::TRUE,
            host_query_reset: false,
        };
        let missing = wants.missing_on(instance.handle(), physical, &features);
        if !missing.is_empty() {
            return Err(Error::Unsupported(format!(
                "the host device was created without {}",
                missing.join(", ")
            )));
        }

        let device = unsafe { ash::Device::load(instance.handle().fp_v1_0(), handles.device) };
        let enabled = |name: &CStr| extensions.iter().any(|enabled| enabled.as_c_str() == name);
        let memory = MemoryFeatures {
            budget: enabled(ext::memory_budget::NAME),
            pageable: false,
        };
        let interop = borrowed_interop(&instance, physical);
        tracing::info!("drawing on the host's GPU: {name}");

        Self::finish(Finish {
            instance,
            physical,
            queue_family: handles.queue_family,
            device,
            graphics: LockedQueue::new(handles.queue),
            background: None,
            transfer: None,
            extensions,
            video: None,
            interop,
            memory,
            wants,
            ray_tracing: false,
            faults: false,
            ownership: Ownership::Borrowed,
            host: Some(submit),
        })
    }
}

#[cfg(unix)]
fn borrowed_interop(instance: &VulkanInstance, physical: vk::PhysicalDevice) -> Interop {
    Interop {
        dmabuf: None,
        drm_nodes: device_extensions(instance, physical)
            .and_then(|offered| dmabuf_support::drm_nodes(instance, physical, &offered)),
    }
}

#[cfg(windows)]
fn borrowed_interop(_instance: &VulkanInstance, _physical: vk::PhysicalDevice) -> Interop {
    Interop { win32: None }
}

fn refuse_beside_external(request: DeviceRequest) -> Result<()> {
    let refused = [
        #[cfg(unix)]
        (request.drm_node.is_some(), "a DRM node"),
        (request.background, "background compute"),
        (request.video.any(), "video coding"),
        (request.prioritized, "latency-critical queues"),
        (request.transfer, "an upload queue"),
        (request.ray_tracing, "ray tracing"),
    ];
    match refused.iter().find(|(asked, _)| *asked) {
        Some((_, what)) => Err(Error::Invalid(format!(
            "{what} needs a device the renderer creates, not an external one"
        ))),
        None => Ok(()),
    }
}
