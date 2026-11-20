use std::collections::HashSet;
use std::ffi::{CStr, CString};

use ash::vk;

use crate::core::instance::VulkanInstance;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrmNodeIds {
    pub primary: Option<(u32, u32)>,
    pub render: Option<(u32, u32)>,
}

impl DrmNodeIds {
    pub fn matches(&self, node: (u32, u32)) -> bool {
        self.primary == Some(node) || self.render == Some(node)
    }
}

const MEMORY_EXTENSIONS: [&CStr; 4] = [
    vk::KHR_EXTERNAL_MEMORY_FD_NAME,
    vk::EXT_EXTERNAL_MEMORY_DMA_BUF_NAME,
    vk::EXT_IMAGE_DRM_FORMAT_MODIFIER_NAME,
    vk::EXT_QUEUE_FAMILY_FOREIGN_NAME,
];

#[derive(Clone, Debug)]
pub struct DmabufSupport {
    pub extensions: Vec<&'static CStr>,
    pub sync_file_export: bool,
    pub sync_file_import: bool,
}

impl DmabufSupport {
    pub(crate) fn sync_file(&self) -> bool {
        self.sync_file_export || self.sync_file_import
    }
}

pub(crate) fn probe(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
    available: &HashSet<CString>,
) -> Option<DmabufSupport> {
    if !MEMORY_EXTENSIONS
        .iter()
        .all(|name| available.contains(*name))
    {
        return None;
    }

    let features = match available.contains(vk::KHR_EXTERNAL_SEMAPHORE_FD_NAME) {
        true => sync_file_features(instance, physical),
        false => vk::ExternalSemaphoreFeatureFlags::empty(),
    };
    let sync_file_export = features.contains(vk::ExternalSemaphoreFeatureFlags::EXPORTABLE);
    let sync_file_import = features.contains(vk::ExternalSemaphoreFeatureFlags::IMPORTABLE);

    let mut extensions = MEMORY_EXTENSIONS.to_vec();
    if sync_file_export || sync_file_import {
        extensions.push(vk::KHR_EXTERNAL_SEMAPHORE_FD_NAME);
    }

    Some(DmabufSupport {
        extensions,
        sync_file_export,
        sync_file_import,
    })
}

fn sync_file_features(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
) -> vk::ExternalSemaphoreFeatureFlags {
    let info = vk::PhysicalDeviceExternalSemaphoreInfo::default()
        .handle_type(vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD);
    let mut properties = vk::ExternalSemaphoreProperties::default();
    unsafe {
        instance
            .handle()
            .get_physical_device_external_semaphore_properties(physical, &info, &mut properties);
    }
    properties.external_semaphore_features
}

pub(crate) fn drm_nodes(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
    available: &HashSet<CString>,
) -> Option<DrmNodeIds> {
    if !available.contains(vk::EXT_PHYSICAL_DEVICE_DRM_NAME) {
        return None;
    }

    let mut drm = vk::PhysicalDeviceDrmPropertiesEXT::default();
    let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut drm);
    unsafe {
        instance
            .handle()
            .get_physical_device_properties2(physical, &mut properties);
    }

    Some(DrmNodeIds {
        primary: (drm.has_primary == vk::TRUE)
            .then_some((drm.primary_major as u32, drm.primary_minor as u32)),
        render: (drm.has_render == vk::TRUE)
            .then_some((drm.render_major as u32, drm.render_minor as u32)),
    })
}
