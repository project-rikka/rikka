use std::ffi::{CStr, CString};
use std::sync::Arc;

use ash::vk;

use crate::core::caps::FeatureChain;

pub trait HostSubmit: Send + Sync + 'static {
    fn submit(&self, batches: &[vk::SubmitInfo2<'_>]) -> Result<(), vk::Result>;
}

#[derive(Clone, Copy, Debug)]
pub struct ExternalHandles {
    pub get_instance_proc_addr: vk::PFN_vkGetInstanceProcAddr,
    pub instance: vk::Instance,
    pub api_version: u32,
    pub physical_device: vk::PhysicalDevice,
    pub device: vk::Device,
    pub queue: vk::Queue,
    pub queue_family: u32,
}

pub struct ExternalDevice {
    pub(crate) handles: ExternalHandles,
    pub(crate) instance_extensions: Vec<CString>,
    pub(crate) device_extensions: Vec<CString>,
    pub(crate) features: FeatureChain,
    pub(crate) submit: Arc<dyn HostSubmit>,
}

impl ExternalDevice {
    pub unsafe fn new(handles: ExternalHandles, submit: impl HostSubmit) -> Self {
        Self {
            handles,
            instance_extensions: Vec::new(),
            device_extensions: Vec::new(),
            features: FeatureChain::default(),
            submit: Arc::new(submit),
        }
    }

    pub fn instance_extension(mut self, name: &CStr) -> Self {
        self.instance_extensions.push(name.to_owned());
        self
    }

    pub fn device_extension(mut self, name: &CStr) -> Self {
        self.device_extensions.push(name.to_owned());
        self
    }

    pub unsafe fn enabled_features(mut self, features: &vk::PhysicalDeviceFeatures2<'_>) -> Self {
        self.features = unsafe { FeatureChain::enabled_in(features) };
        self
    }
}

impl std::fmt::Debug for ExternalDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalDevice")
            .field("handles", &self.handles)
            .field("instance_extensions", &self.instance_extensions)
            .field("device_extensions", &self.device_extensions)
            .finish_non_exhaustive()
    }
}
