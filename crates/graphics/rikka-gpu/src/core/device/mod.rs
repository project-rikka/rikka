mod creation;
mod queues;
mod request;
mod selection;

use std::ffi::CString;

#[cfg(unix)]
use ash::khr;
use ash::{ext, vk};

use crate::core::caps::DeviceProperties;
#[cfg(unix)]
use crate::core::dmabuf_support::{DmabufSupport, DrmNodeIds};
use crate::core::fault::FaultReport;
use crate::core::instance::VulkanInstance;
use crate::core::memory::{Allocation, DeviceMemory, HeapBudget, Placement};
use crate::core::pipeline_cache::PipelineCache;
use crate::core::queue_lock::LockedQueue;
use crate::core::ray_support::RayTracing;
use crate::core::video_support::VideoSupport;
use crate::error::Result;
#[cfg(windows)]
use crate::win32::Win32Device;

use queues::FamilyQueue;

pub use request::DeviceRequest;

pub struct RenderDevice {
    instance: VulkanInstance,
    physical: vk::PhysicalDevice,
    queue_family: u32,
    device: ash::Device,
    graphics: LockedQueue,
    background: Option<FamilyQueue>,
    transfer: Option<FamilyQueue>,
    memory: DeviceMemory,
    pipeline_cache: PipelineCache,
    properties: DeviceProperties,
    extensions: Vec<CString>,
    video: Option<VideoSupport>,
    interop: Interop,
    three_d: bool,
    compute: bool,
    anisotropy: bool,
    resets_queries_on_host: bool,
    debug: Option<ext::debug_utils::Device>,
    ray_tracing: Option<RayTracing>,
    fault: FaultReport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueRole {
    Graphics,
    Background,
    Transfer,
}

#[cfg(unix)]
pub(crate) struct Interop {
    pub(crate) dmabuf: Option<DmabufDevice>,
    pub(crate) drm_nodes: Option<DrmNodeIds>,
}

#[cfg(windows)]
pub(crate) struct Interop {
    pub(crate) win32: Option<Win32Device>,
}

#[cfg(unix)]
pub struct DmabufDevice {
    pub support: DmabufSupport,
    pub memory_fd: khr::external_memory_fd::Device,
    pub modifiers: ext::image_drm_format_modifier::Device,
    pub semaphore_fd: Option<khr::external_semaphore_fd::Device>,
}

impl RenderDevice {
    pub fn instance(&self) -> &VulkanInstance {
        &self.instance
    }

    pub fn physical(&self) -> vk::PhysicalDevice {
        self.physical
    }

    pub fn handle(&self) -> &ash::Device {
        &self.device
    }

    pub(crate) fn graphics_queue(&self) -> &LockedQueue {
        &self.graphics
    }

    pub fn queue_family(&self) -> u32 {
        self.queue_family
    }

    pub(crate) fn background_queue(&self) -> (&LockedQueue, u32) {
        match &self.background {
            Some(background) => (&background.queue, background.family),
            None => (&self.graphics, self.queue_family),
        }
    }

    pub(crate) fn transfer_family(&self) -> Option<u32> {
        self.transfer.as_ref().map(|transfer| transfer.family)
    }

    fn queue(&self, role: QueueRole) -> &LockedQueue {
        match role {
            QueueRole::Graphics => &self.graphics,
            QueueRole::Background => self.background_queue().0,
            QueueRole::Transfer => self
                .transfer
                .as_ref()
                .map_or(&self.graphics, |transfer| &transfer.queue),
        }
    }

    pub fn submit(
        &self,
        role: QueueRole,
        batches: &[vk::SubmitInfo2<'_>],
        fence: vk::Fence,
    ) -> Result<()> {
        self.queue(role)
            .submit(&self.device, batches, fence)
            .inspect_err(|result| self.observe_loss(*result))?;
        Ok(())
    }

    pub(crate) fn observe_loss(&self, result: vk::Result) {
        self.fault.observe(result);
    }

    pub fn pipeline_cache(&self) -> vk::PipelineCache {
        self.pipeline_cache.handle()
    }

    pub fn save_pipeline_cache_in_background(&self) {
        crate::core::pipeline::save_in_background(self.pipeline_cache.persisted());
    }

    pub fn save_pipeline_cache(&self) -> Result<()> {
        self.pipeline_cache.save()
    }

    pub fn limits(&self) -> &vk::PhysicalDeviceLimits {
        &self.properties.limits
    }

    pub(crate) fn table_capacity(&self) -> u32 {
        self.properties.table_capacity
    }

    pub(crate) fn resolves_msaa_depth(&self) -> bool {
        self.properties
            .depth_resolve_modes
            .contains(vk::ResolveModeFlags::MIN)
    }

    pub fn depth_resolve_mode(&self) -> vk::ResolveModeFlags {
        vk::ResolveModeFlags::MIN
    }

    pub fn storage_image_capacity(&self) -> u32 {
        self.properties.storage_image_capacity
    }

    pub fn three_d(&self) -> bool {
        self.three_d
    }

    pub(crate) fn computes(&self) -> bool {
        self.compute
    }

    pub fn ray_tracing(&self) -> Option<&RayTracing> {
        self.ray_tracing.as_ref()
    }

    pub(crate) fn filters_anisotropically(&self) -> bool {
        self.anisotropy
    }

    pub fn resets_queries_on_host(&self) -> bool {
        self.resets_queries_on_host
    }
    pub fn sample_count(&self, requested: u32) -> vk::SampleCountFlags {
        if !self.resolves_msaa_depth() {
            return vk::SampleCountFlags::TYPE_1;
        }
        let supported = self.properties.sample_counts;
        [
            (16, vk::SampleCountFlags::TYPE_16),
            (8, vk::SampleCountFlags::TYPE_8),
            (4, vk::SampleCountFlags::TYPE_4),
            (2, vk::SampleCountFlags::TYPE_2),
        ]
        .into_iter()
        .find(|&(count, flag)| count <= requested && supported.contains(flag))
        .map_or(vk::SampleCountFlags::TYPE_1, |(_, flag)| flag)
    }

    pub(crate) fn extensions(&self) -> &[CString] {
        &self.extensions
    }

    pub fn vendor_id(&self) -> u32 {
        self.properties.vendor_id
    }

    pub(crate) fn video(&self) -> Option<&VideoSupport> {
        self.video.as_ref()
    }

    #[cfg(unix)]
    pub fn dmabuf(&self) -> Option<&DmabufDevice> {
        self.interop.dmabuf.as_ref()
    }

    #[cfg(unix)]
    pub fn drm_nodes(&self) -> Option<DrmNodeIds> {
        self.interop.drm_nodes
    }

    #[cfg(windows)]
    pub(crate) fn win32(&self) -> Option<&Win32Device> {
        self.interop.win32.as_ref()
    }

    pub fn set_object_name<H: vk::Handle>(&self, handle: H, name: &str) {
        let Some(debug) = &self.debug else { return };
        let Ok(name) = CString::new(name) else { return };
        let info = vk::DebugUtilsObjectNameInfoEXT::default()
            .object_handle(handle)
            .object_name(&name);
        let _ = unsafe { debug.set_debug_utils_object_name(&info) };
    }

    pub(crate) fn allocate_image(
        &self,
        name: &str,
        image: vk::Image,
        usage: vk::ImageUsageFlags,
    ) -> Result<Allocation> {
        self.memory.allocate_image(name, image, usage)
    }

    pub fn allocate_heap(
        &self,
        name: &str,
        requirements: vk::MemoryRequirements,
    ) -> Result<Allocation> {
        self.memory.allocate_heap(name, requirements)
    }

    pub(crate) fn allocate_buffer(
        &self,
        name: &str,
        buffer: vk::Buffer,
        placement: Placement,
    ) -> Result<Allocation> {
        self.memory.allocate_buffer(name, buffer, placement)
    }

    pub fn free(&self, allocation: Allocation) {
        self.memory.free(allocation);
    }

    pub fn report_memory(&self) {
        self.memory.report_usage();
    }

    pub fn memory_budget(&self) -> Option<Vec<HeapBudget>> {
        self.memory.budget()
    }

    pub fn wait_idle(&self) {
        let _graphics = self.graphics.guard();
        let _background = self
            .background
            .as_ref()
            .map(|background| background.queue.guard());
        let _transfer = self
            .transfer
            .as_ref()
            .map(|transfer| transfer.queue.guard());
        let result = unsafe { self.device.device_wait_idle() };
        if let Err(err) = result {
            tracing::error!("vkDeviceWaitIdle failed: {err}");
        }
    }

    pub(crate) fn debug_utils(&self) -> Option<&ext::debug_utils::Device> {
        self.debug.as_ref()
    }
}

impl Drop for RenderDevice {
    fn drop(&mut self) {
        self.wait_idle();
        unsafe {
            self.pipeline_cache.close();
            self.memory.close();
            self.device.destroy_device(None);
        }
    }
}

impl std::fmt::Debug for RenderDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderDevice").finish_non_exhaustive()
    }
}

#[cfg(unix)]
impl std::fmt::Debug for DmabufDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DmabufDevice").finish_non_exhaustive()
    }
}
