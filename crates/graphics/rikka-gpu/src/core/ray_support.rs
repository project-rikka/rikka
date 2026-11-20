use std::collections::HashSet;
use std::ffi::{CStr, CString};

use ash::{khr, vk};

use crate::core::instance::VulkanInstance;

pub(crate) const EXTENSIONS: [&CStr; 3] = [
    khr::acceleration_structure::NAME,
    khr::ray_query::NAME,
    khr::deferred_host_operations::NAME,
];

pub struct RayTracing {
    pub structures: khr::acceleration_structure::Device,
    pub scratch_alignment: u64,
}

impl RayTracing {
    pub(crate) fn new(
        instance: &VulkanInstance,
        physical: vk::PhysicalDevice,
        device: &ash::Device,
    ) -> Self {
        let mut structures = vk::PhysicalDeviceAccelerationStructurePropertiesKHR::default();
        let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut structures);
        unsafe {
            instance
                .handle()
                .get_physical_device_properties2(physical, &mut properties)
        };
        Self {
            structures: khr::acceleration_structure::Device::new(instance.handle(), device),
            scratch_alignment: u64::from(
                structures
                    .min_acceleration_structure_scratch_offset_alignment
                    .max(1),
            ),
        }
    }
}

pub(crate) fn supported(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
    available: &HashSet<CString>,
) -> bool {
    if !EXTENSIONS.iter().all(|name| available.contains(*name)) {
        return false;
    }
    let mut structures = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default();
    let mut query = vk::PhysicalDeviceRayQueryFeaturesKHR::default();
    let mut features = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut structures)
        .push_next(&mut query);
    unsafe {
        instance
            .handle()
            .get_physical_device_features2(physical, &mut features)
    };
    structures.acceleration_structure == vk::TRUE && query.ray_query == vk::TRUE
}

impl std::fmt::Debug for RayTracing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RayTracing").finish_non_exhaustive()
    }
}
