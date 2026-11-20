use std::collections::HashSet;
use std::ffi::{CStr, CString, FromBytesUntilNulError};
use std::sync::atomic::{AtomicBool, Ordering};

use ash::{ext, vk};

use crate::core::instance::VulkanInstance;

pub(crate) fn supported(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
    extensions: &HashSet<CString>,
) -> bool {
    if !extensions.contains(ext::device_fault::NAME) {
        return false;
    }
    let mut fault = vk::PhysicalDeviceFaultFeaturesEXT::default();
    let mut features = vk::PhysicalDeviceFeatures2::default().push_next(&mut fault);
    unsafe {
        instance
            .handle()
            .get_physical_device_features2(physical, &mut features)
    };
    fault.device_fault == vk::TRUE
}

pub(crate) struct FaultReport {
    loader: Option<ext::device_fault::Device>,
    gpu: String,
    reported: AtomicBool,
}

impl FaultReport {
    pub(crate) fn new(
        instance: &VulkanInstance,
        physical: vk::PhysicalDevice,
        device: &ash::Device,
        enabled: bool,
    ) -> Self {
        Self {
            loader: enabled.then(|| ext::device_fault::Device::new(instance.handle(), device)),
            gpu: gpu_name(instance, physical),
            reported: AtomicBool::new(false),
        }
    }

    pub(crate) fn observe(&self, result: vk::Result) {
        if result != vk::Result::ERROR_DEVICE_LOST || self.reported.swap(true, Ordering::AcqRel) {
            return;
        }
        let gpu = &self.gpu;
        let Some(loader) = &self.loader else {
            tracing::error!(%gpu, "GPU device lost; the driver offers no fault report");
            return;
        };
        match describe(loader) {
            Ok(report) => tracing::error!(%gpu, "GPU device lost: {report}"),
            Err(error) => {
                tracing::error!(%gpu, "GPU device lost; failed to read the fault report: {error}")
            }
        }
    }
}

fn gpu_name(instance: &VulkanInstance, physical: vk::PhysicalDevice) -> String {
    let mut driver = vk::PhysicalDeviceDriverProperties::default();
    let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut driver);
    unsafe {
        instance
            .handle()
            .get_physical_device_properties2(physical, &mut properties)
    };
    format!(
        "{} ({} {})",
        text(properties.properties.device_name_as_c_str()),
        text(driver.driver_name_as_c_str()),
        text(driver.driver_info_as_c_str())
    )
}

impl std::fmt::Debug for FaultReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FaultReport")
            .field("enabled", &self.loader.is_some())
            .finish_non_exhaustive()
    }
}

fn describe(loader: &ext::device_fault::Device) -> Result<String, vk::Result> {
    let read = loader.fp().get_device_fault_info_ext;
    let mut counts = vk::DeviceFaultCountsEXT::default();
    unsafe { read(loader.device(), &mut counts, std::ptr::null_mut()) }.result()?;
    let mut addresses =
        vec![vk::DeviceFaultAddressInfoEXT::default(); counts.address_info_count as usize];
    let mut vendors =
        vec![vk::DeviceFaultVendorInfoEXT::default(); counts.vendor_info_count as usize];
    counts.vendor_binary_size = 0;
    let mut info = vk::DeviceFaultInfoEXT {
        p_address_infos: addresses.as_mut_ptr(),
        p_vendor_infos: vendors.as_mut_ptr(),
        ..Default::default()
    };
    match unsafe { read(loader.device(), &mut counts, &mut info) } {
        vk::Result::SUCCESS | vk::Result::INCOMPLETE => {}
        failed => return Err(failed),
    }
    addresses.truncate(counts.address_info_count as usize);
    vendors.truncate(counts.vendor_info_count as usize);
    let faults = addresses.iter().map(|address| {
        format!(
            "{:?} at {:#x} (±{:#x})",
            address.address_type, address.reported_address, address.address_precision
        )
    });
    let codes = vendors.iter().map(|vendor| {
        format!(
            "{} (code {:#x}, data {:#x})",
            text(vendor.description_as_c_str()),
            vendor.vendor_fault_code,
            vendor.vendor_fault_data
        )
    });
    Ok(std::iter::once(text(info.description_as_c_str()))
        .chain(faults)
        .chain(codes)
        .collect::<Vec<_>>()
        .join("; "))
}

fn text(name: Result<&CStr, FromBytesUntilNulError>) -> String {
    name.map_or_else(
        |_| String::new(),
        |name| name.to_string_lossy().into_owned(),
    )
}
