use std::collections::HashSet;
use std::ffi::{CStr, CString};

use ash::{ext, khr, vk};

use crate::core::caps::{FeatureChain, FeatureRequest};
#[cfg(unix)]
use crate::core::dmabuf_support::{self, DmabufSupport, DrmNodeIds};
use crate::core::fault;
use crate::core::instance::VulkanInstance;
use crate::error::{Error, Result};
#[cfg(windows)]
use crate::win32;

use super::RenderDevice;
use super::request::DeviceRequest;

#[cfg(windows)]
const PREFERRED_SCORE: u32 = 1000;

#[cfg(windows)]
const PRESENTS_SCORE: u32 = 10_000;

pub(crate) fn device_extensions(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
) -> Option<HashSet<CString>> {
    let properties = unsafe {
        instance
            .handle()
            .enumerate_device_extension_properties(physical)
    }
    .ok()?;
    Some(
        properties
            .iter()
            .filter_map(|ext| ext.extension_name_as_c_str().ok())
            .map(CStr::to_owned)
            .collect(),
    )
}

pub(super) struct Candidate {
    pub(super) physical: vk::PhysicalDevice,
    pub(super) queue_family: u32,
    pub(super) queue_count: u32,
    score: u32,
    pub(super) name: String,
    #[cfg(unix)]
    pub(super) dmabuf: Option<DmabufSupport>,
    #[cfg(unix)]
    pub(super) drm_nodes: Option<DrmNodeIds>,
    #[cfg(windows)]
    pub(super) presentation: Option<win32::Presentation>,
    #[cfg(windows)]
    pub(super) luid: Option<[u8; 8]>,
    pub(super) features: FeatureChain,
    pub(super) extensions: HashSet<CString>,
    pub(super) faults: bool,
}

#[cfg(windows)]
impl Candidate {
    fn ranked_by(mut self, preferred: Option<[u8; 8]>) -> Self {
        if preferred.is_some() && self.luid == preferred {
            self.score += PREFERRED_SCORE;
        }
        if self.presentation.is_some() {
            self.score += PRESENTS_SCORE;
        }
        self
    }
}

pub(super) fn global_priority(extensions: &HashSet<CString>) -> Option<&'static CStr> {
    [khr::global_priority::NAME, ext::global_priority::NAME]
        .into_iter()
        .find(|name| extensions.contains(*name))
}

pub(super) fn supports_pageable_memory(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
) -> bool {
    let mut priority = vk::PhysicalDeviceMemoryPriorityFeaturesEXT::default();
    let mut pageable = vk::PhysicalDevicePageableDeviceLocalMemoryFeaturesEXT::default();
    let mut features = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut priority)
        .push_next(&mut pageable);
    unsafe {
        instance
            .handle()
            .get_physical_device_features2(physical, &mut features)
    };
    priority.memory_priority == vk::TRUE && pageable.pageable_device_local_memory == vk::TRUE
}

impl RenderDevice {
    pub(super) fn pick_physical_device(
        instance: &VulkanInstance,
        request: DeviceRequest,
    ) -> Result<Candidate> {
        let physical_devices = unsafe { instance.handle().enumerate_physical_devices()? };

        let mut best: Option<Candidate> = None;
        let mut rejections = Vec::new();
        let mut lacking_mesh = Vec::new();
        #[cfg(windows)]
        let preferred = win32::preferred_adapter();
        for physical in physical_devices {
            match Self::evaluate(instance, physical, request) {
                Ok(candidate) => {
                    #[cfg(windows)]
                    let candidate = candidate.ranked_by(preferred);
                    let missing = match request.three_d {
                        true => FeatureRequest::plain(true).missing_on(
                            instance.handle(),
                            physical,
                            &candidate.features,
                        ),
                        false => Vec::new(),
                    };
                    if !missing.is_empty() {
                        lacking_mesh.push(format!(
                            "{}: missing {}",
                            candidate.name,
                            missing.join(", ")
                        ));
                    } else if best.as_ref().is_none_or(|b| candidate.score > b.score) {
                        best = Some(candidate);
                    }
                }
                Err(reason) => rejections.push(reason),
            }
        }

        best.ok_or_else(|| match (lacking_mesh.is_empty(), rejections.is_empty()) {
            (false, _) => Error::Unsupported(format!(
                "no GPU supports 3D rendering ({})",
                lacking_mesh.join("; ")
            )),
            (true, true) => Error::NoSuitableDevice("no Vulkan devices found".to_owned()),
            (true, false) => Error::NoSuitableDevice(rejections.join("; ")),
        })
    }

    #[cfg_attr(
        windows,
        expect(unused_variables, reason = "only DRM requests constrain the device")
    )]
    fn evaluate(
        instance: &VulkanInstance,
        physical: vk::PhysicalDevice,
        request: DeviceRequest,
    ) -> std::result::Result<Candidate, String> {
        let properties = unsafe { instance.handle().get_physical_device_properties(physical) };
        let name = properties
            .device_name_as_c_str()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "<unknown>".to_owned());

        if properties.api_version < vk::API_VERSION_1_3 {
            return Err(format!("{name}: Vulkan 1.3 not supported"));
        }

        let available = FeatureChain::query(instance.handle(), physical);
        let missing =
            FeatureRequest::plain(false).missing_on(instance.handle(), physical, &available);
        if !missing.is_empty() {
            return Err(format!("{name}: missing {}", missing.join(", ")));
        }

        let extensions = device_extensions(instance, physical)
            .ok_or_else(|| format!("{name}: failed to enumerate device extensions"))?;

        let queue_families = unsafe {
            instance
                .handle()
                .get_physical_device_queue_family_properties(physical)
        };
        let (queue_family, family) = queue_families
            .iter()
            .enumerate()
            .find(|(_, family)| family.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .ok_or_else(|| format!("{name}: no graphics queue"))?;

        #[cfg(unix)]
        let drm_nodes = dmabuf_support::drm_nodes(instance, physical, &extensions);
        #[cfg(unix)]
        let dmabuf = dmabuf_support::probe(instance, physical, &extensions);
        #[cfg(unix)]
        if let Some(requested) = request.drm_node {
            let Some(nodes) = drm_nodes else {
                return Err(format!("{name}: VK_EXT_physical_device_drm not supported"));
            };
            if !nodes.matches(requested) {
                return Err(format!("{name}: not the requested DRM device"));
            }
            if dmabuf.is_none() {
                return Err(format!("{name}: cannot import dmabufs"));
            }
        }

        let score = match properties.device_type {
            vk::PhysicalDeviceType::DISCRETE_GPU => 100,
            vk::PhysicalDeviceType::INTEGRATED_GPU => 50,
            vk::PhysicalDeviceType::VIRTUAL_GPU => 25,
            _ => 10,
        };

        Ok(Candidate {
            physical,
            queue_family: queue_family as u32,
            queue_count: family.queue_count,
            score,
            name,
            #[cfg(unix)]
            dmabuf,
            #[cfg(unix)]
            drm_nodes,
            #[cfg(windows)]
            presentation: win32::Presentation::probe(
                instance,
                physical,
                queue_family as u32,
                &extensions,
            ),
            #[cfg(windows)]
            luid: win32::adapter_luid(instance, physical),
            features: available,
            faults: fault::supported(instance, physical, &extensions),
            extensions,
        })
    }
}
