use std::sync::{Arc, OnceLock};

use crate::compute::Compute;
use crate::core::device::{DeviceRequest, RenderDevice};
#[cfg(unix)]
use crate::core::dmabuf_support::DrmNodeIds;
use crate::core::instance::VulkanInstance;
#[cfg(unix)]
use crate::drm::{DrmFormat, query_export_formats, query_render_formats, query_texture_formats};
use crate::error::Result;
use crate::video::{VideoDeviceHandles, video_handles};

#[derive(Clone)]
pub struct Gpu {
    shared: Arc<Shared>,
}

struct Shared {
    device: Arc<RenderDevice>,
    compute: OnceLock<Compute>,
    #[cfg(unix)]
    render_formats: OnceLock<Vec<DrmFormat>>,
    #[cfg(unix)]
    texture_formats: OnceLock<Vec<DrmFormat>>,
    #[cfg(unix)]
    export_formats: OnceLock<Vec<DrmFormat>>,
}

impl std::fmt::Debug for Gpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gpu")
            .field("device", &self.shared.device)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
#[must_use]
pub struct GpuBuilder {
    app_name: String,
    validation: bool,
    request: DeviceRequest,
}

impl GpuBuilder {
    pub fn app_name(mut self, name: impl Into<String>) -> Self {
        self.app_name = name.into();
        self
    }

    pub fn validation(mut self, enabled: bool) -> Self {
        self.validation = enabled;
        self
    }

    #[cfg(unix)]
    pub fn drm_node(mut self, major: u32, minor: u32) -> Self {
        self.request.drm_node = Some((major, minor));
        self
    }

    pub fn with_3d(mut self) -> Self {
        self.request.three_d = true;
        self
    }

    pub fn compute_support(mut self) -> Self {
        self.request.compute = true;
        self
    }

    pub fn background_compute(mut self) -> Self {
        self.request.compute = true;
        self.request.background = true;
        self
    }

    pub fn video_decode(mut self) -> Self {
        self.request.video.decode = true;
        self
    }

    pub fn video_encode(mut self) -> Self {
        self.request.video.encode = true;
        self
    }

    pub fn latency_critical(mut self) -> Self {
        self.request.prioritized = true;
        self
    }

    pub fn upload_queue(mut self) -> Self {
        self.request.transfer = true;
        self
    }

    pub fn ray_tracing(mut self) -> Self {
        self.request.three_d = true;
        self.request.ray_tracing = true;
        self
    }

    pub fn build(self) -> Result<Gpu> {
        let instance = VulkanInstance::new(&self.app_name, self.validation)?;
        let device = RenderDevice::new(instance, self.request)?;
        Ok(Gpu {
            shared: Arc::new(Shared {
                device: Arc::new(device),
                compute: OnceLock::new(),
                #[cfg(unix)]
                render_formats: OnceLock::new(),
                #[cfg(unix)]
                texture_formats: OnceLock::new(),
                #[cfg(unix)]
                export_formats: OnceLock::new(),
            }),
        })
    }
}

impl Gpu {
    pub fn builder() -> GpuBuilder {
        GpuBuilder {
            app_name: "rikka".to_owned(),
            validation: cfg!(feature = "validation"),
            request: DeviceRequest::default(),
        }
    }

    pub fn compute(&self) -> Result<Compute> {
        if let Some(compute) = self.shared.compute.get() {
            return Ok(compute.clone());
        }
        let compute = Compute::new(self.shared.device.clone())?;
        Ok(self.shared.compute.get_or_init(|| compute).clone())
    }

    #[must_use]
    pub fn video_device(&self) -> Option<VideoDeviceHandles> {
        video_handles(&self.shared.device)
    }

    #[must_use]
    pub fn vendor_id(&self) -> u32 {
        self.shared.device.vendor_id()
    }

    #[cfg(unix)]
    #[must_use]
    pub fn drm_nodes(&self) -> Option<DrmNodeIds> {
        self.shared.device.drm_nodes()
    }

    #[must_use]
    pub fn traces_rays(&self) -> bool {
        self.shared.device.ray_tracing().is_some()
    }

    #[cfg(unix)]
    #[must_use]
    pub fn dmabuf_render_formats(&self) -> &[DrmFormat] {
        self.dmabuf_formats(&self.shared.render_formats, query_render_formats)
    }

    #[cfg(unix)]
    #[must_use]
    pub fn dmabuf_texture_formats(&self) -> &[DrmFormat] {
        self.dmabuf_formats(&self.shared.texture_formats, query_texture_formats)
    }

    #[cfg(unix)]
    #[must_use]
    pub fn dmabuf_export_formats(&self) -> &[DrmFormat] {
        self.dmabuf_formats(&self.shared.export_formats, query_export_formats)
    }

    pub fn save_pipeline_cache(&self) -> Result<()> {
        self.shared.device.save_pipeline_cache()
    }

    pub fn trim(&self) {
        if let Some(compute) = self.shared.compute.get() {
            compute.trim();
        }
        self.shared.device.save_pipeline_cache_in_background();
        self.shared.device.report_memory();
    }

    #[doc(hidden)]
    #[must_use]
    pub fn device(&self) -> &Arc<RenderDevice> {
        &self.shared.device
    }

    #[cfg(unix)]
    fn dmabuf_formats<'a>(
        &self,
        cache: &'a OnceLock<Vec<DrmFormat>>,
        query: fn(&VulkanInstance, ash::vk::PhysicalDevice) -> Vec<DrmFormat>,
    ) -> &'a [DrmFormat] {
        cache.get_or_init(|| {
            let device = &self.shared.device;
            if device.dmabuf().is_none() {
                return Vec::new();
            }
            query(device.instance(), device.physical())
        })
    }
}
