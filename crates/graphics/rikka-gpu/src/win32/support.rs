use std::collections::HashSet;
use std::ffi::{CStr, CString};

use ash::{khr, vk};

use crate::core::instance::VulkanInstance;

pub(crate) const SURFACE_EXTENSIONS: [&CStr; 2] = [khr::surface::NAME, khr::win32_surface::NAME];

#[derive(Clone, Copy, Debug)]
pub(crate) struct Presentation {
    incremental: bool,
}

impl Presentation {
    pub(crate) fn probe(
        instance: &VulkanInstance,
        physical: vk::PhysicalDevice,
        family: u32,
        available: &HashSet<CString>,
    ) -> Option<Presentation> {
        let surfaces = SURFACE_EXTENSIONS
            .iter()
            .all(|name| instance.has_extension(name));
        if !surfaces || !available.contains(khr::swapchain::NAME) {
            return None;
        }
        let loader = khr::win32_surface::Instance::new(instance.entry(), instance.handle());
        let presents =
            unsafe { loader.get_physical_device_win32_presentation_support(physical, family) };
        presents.then(|| Presentation {
            incremental: available.contains(khr::incremental_present::NAME),
        })
    }

    pub(crate) fn extensions(self) -> impl Iterator<Item = &'static CStr> {
        [khr::swapchain::NAME]
            .into_iter()
            .chain(self.incremental.then_some(khr::incremental_present::NAME))
    }
}

pub(crate) struct Win32Device {
    pub(crate) surface: khr::surface::Instance,
    pub(crate) win32_surface: khr::win32_surface::Instance,
    pub(crate) swapchain: khr::swapchain::Device,
    pub(crate) incremental_present: bool,
}

impl Win32Device {
    pub(crate) fn new(
        instance: &VulkanInstance,
        device: &ash::Device,
        presentation: Presentation,
    ) -> Self {
        Self {
            surface: khr::surface::Instance::new(instance.entry(), instance.handle()),
            win32_surface: khr::win32_surface::Instance::new(instance.entry(), instance.handle()),
            swapchain: khr::swapchain::Device::new(instance.handle(), device),
            incremental_present: presentation.incremental,
        }
    }
}

impl std::fmt::Debug for Win32Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Win32Device")
            .field("incremental_present", &self.incremental_present)
            .finish_non_exhaustive()
    }
}

pub(crate) fn adapter_luid(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
) -> Option<[u8; 8]> {
    let mut id = vk::PhysicalDeviceIDProperties::default();
    let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut id);
    unsafe {
        instance
            .handle()
            .get_physical_device_properties2(physical, &mut properties);
    }
    (id.device_luid_valid == vk::TRUE).then_some(id.device_luid)
}
