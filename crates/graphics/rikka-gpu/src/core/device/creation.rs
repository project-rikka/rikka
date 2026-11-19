use std::ffi::{CStr, CString, c_char};
use std::sync::Arc;

#[cfg(unix)]
use ash::khr;
use ash::{ext, vk};

use crate::core::caps::{DeviceProperties, FeatureChain, FeatureRequest};
#[cfg(unix)]
use crate::core::dmabuf_support::DmabufSupport;
use crate::core::fault::FaultReport;
use crate::core::instance::{Ownership, VulkanInstance};
use crate::core::memory::{DeviceMemory, MemoryFeatures};
use crate::core::pipeline_cache::PipelineCache;
use crate::core::queue_lock::LockedQueue;
use crate::core::ray_support::{self, RayTracing};
use crate::core::video_support::{self, VideoSupport};
use crate::error::Result;
use crate::external::HostSubmit;

#[cfg(unix)]
use super::DmabufDevice;
use super::queues::{FamilyQueue, QueuePlan};
use super::request::DeviceRequest;
use super::selection::{Candidate, global_priority, supports_pageable_memory};
use super::{Interop, RenderDevice};
#[cfg(windows)]
use crate::win32::Win32Device;

pub(super) struct Finish {
    pub(super) instance: VulkanInstance,
    pub(super) physical: vk::PhysicalDevice,
    pub(super) queue_family: u32,
    pub(super) device: ash::Device,
    pub(super) graphics: LockedQueue,
    pub(super) background: Option<FamilyQueue>,
    pub(super) transfer: Option<FamilyQueue>,
    pub(super) extensions: Vec<CString>,
    pub(super) video: Option<VideoSupport>,
    pub(super) interop: Interop,
    pub(super) memory: MemoryFeatures,
    pub(super) wants: FeatureRequest,
    pub(super) ray_tracing: bool,
    pub(super) faults: bool,
    pub(super) ownership: Ownership,
    pub(super) host: Option<Arc<dyn HostSubmit>>,
}

impl RenderDevice {
    pub fn new(instance: VulkanInstance, request: DeviceRequest) -> Result<Self> {
        let candidate = Self::pick_physical_device(&instance, request)?;
        tracing::info!("selected GPU: {}", candidate.name);
        let video = detect_video(&instance, &candidate, request);
        let plan = QueuePlan::new(&instance, &candidate, request, video.as_ref());
        let enabled = EnabledExtensions::negotiate(&instance, &candidate, request, video.as_ref());
        let wants = feature_request(&instance, &candidate, request, plan.transfer.is_some());
        let device = create_device(
            &instance,
            &candidate,
            request,
            &plan.queue_infos(candidate.queue_family, video.as_ref()),
            &enabled,
            wants,
            video
                .as_ref()
                .is_some_and(|support| support.video_maintenance1),
        )?;
        let queues = plan.take_queues(&device, candidate.queue_family);
        let extensions = enabled
            .names
            .iter()
            .map(|&name| unsafe { CStr::from_ptr(name) }.to_owned())
            .collect();
        let interop = open_interop(&instance, &device, &candidate);

        Self::finish(Finish {
            instance,
            physical: candidate.physical,
            queue_family: candidate.queue_family,
            device,
            graphics: queues.graphics,
            background: queues.background,
            transfer: queues.transfer,
            extensions,
            video,
            interop,
            memory: enabled.memory,
            wants,
            ray_tracing: enabled.ray_tracing,
            faults: enabled.faults,
            ownership: Ownership::Owned,
            host: None,
        })
    }

    pub(super) fn finish(parts: Finish) -> Result<Self> {
        let Finish {
            instance,
            physical,
            queue_family,
            device,
            graphics,
            background,
            transfer,
            extensions,
            video,
            interop,
            memory,
            wants,
            ray_tracing,
            faults,
            ownership,
            host,
        } = parts;

        let memory = DeviceMemory::new(
            instance.handle(),
            physical,
            &device,
            memory,
            wants.three_d || wants.compute,
        )?;
        let physical_properties =
            unsafe { instance.handle().get_physical_device_properties(physical) };
        let pipeline_cache = PipelineCache::open(&device, &physical_properties)?;

        let properties = DeviceProperties::query(instance.handle(), physical, wants);
        let debug = instance
            .extensions()
            .iter()
            .any(|name| name.as_c_str() == ext::debug_utils::NAME)
            .then(|| ext::debug_utils::Device::new(instance.handle(), &device));
        let ray_tracing = ray_tracing.then(|| RayTracing::new(&instance, physical, &device));
        let fault = FaultReport::new(&instance, physical, &device, faults);
        let built = Self {
            instance,
            physical,
            queue_family,
            device,
            graphics,
            background,
            transfer,
            memory,
            pipeline_cache,
            properties,
            extensions,
            video,
            interop,
            three_d: wants.three_d,
            compute: wants.compute,
            anisotropy: wants.three_d || wants.anisotropy,
            resets_queries_on_host: wants.host_query_reset,
            debug,
            ray_tracing,
            fault,
            ownership,
            host,
        };
        built.set_object_name(built.graphics.handle(), "rikka.queue.graphics");
        if let Some(background) = &built.background {
            built.set_object_name(background.queue.handle(), "rikka.queue.background");
        }
        if let Some(transfer) = &built.transfer {
            built.set_object_name(transfer.queue.handle(), "rikka.queue.transfer");
        }
        built.set_object_name(built.pipeline_cache.handle(), "rikka.pipeline_cache");
        Ok(built)
    }
}

struct EnabledExtensions {
    names: Vec<*const c_char>,
    memory: MemoryFeatures,
    ray_tracing: bool,
    faults: bool,
}

impl EnabledExtensions {
    fn negotiate(
        instance: &VulkanInstance,
        candidate: &Candidate,
        request: DeviceRequest,
        video: Option<&VideoSupport>,
    ) -> EnabledExtensions {
        let offered = &candidate.extensions;
        let mut names: Vec<*const c_char> = Vec::new();
        if let Some(support) = video {
            names.extend(support.extensions.iter().map(|name| name.as_ptr()));
        }
        #[cfg(unix)]
        if let Some(support) = &candidate.dmabuf {
            names.extend(support.extensions.iter().map(|name| name.as_ptr()));
        }
        #[cfg(windows)]
        if let Some(presentation) = candidate.presentation {
            names.extend(presentation.extensions().map(CStr::as_ptr));
        }
        let memory = MemoryFeatures {
            budget: offered.contains(ext::memory_budget::NAME),
            pageable: offered.contains(ext::memory_priority::NAME)
                && offered.contains(ext::pageable_device_local_memory::NAME)
                && supports_pageable_memory(instance, candidate.physical),
        };
        if memory.budget {
            names.push(ext::memory_budget::NAME.as_ptr());
        }
        if memory.pageable {
            names.push(ext::memory_priority::NAME.as_ptr());
            names.push(ext::pageable_device_local_memory::NAME.as_ptr());
        }
        let ray_tracing = request.ray_tracing
            && request.three_d
            && ray_support::supported(instance, candidate.physical, offered);
        if request.ray_tracing && !ray_tracing {
            tracing::info!("{} does not support ray queries", candidate.name);
        }
        if ray_tracing {
            names.extend(ray_support::EXTENSIONS.iter().map(|name| name.as_ptr()));
        }
        if candidate.faults {
            names.push(ext::device_fault::NAME.as_ptr());
        }
        EnabledExtensions {
            names,
            memory,
            ray_tracing,
            faults: candidate.faults,
        }
    }
}

fn detect_video(
    instance: &VulkanInstance,
    candidate: &Candidate,
    request: DeviceRequest,
) -> Option<VideoSupport> {
    if !request.video.any() {
        return None;
    }
    let video = video_support::detect(
        instance,
        candidate.physical,
        candidate.queue_family,
        &candidate.extensions,
        request.video,
    );
    match &video {
        Some(support) => tracing::info!(
            "Vulkan Video enabled: {} extensions, {} extra queue families",
            support.extensions.len(),
            support.families.len()
        ),
        None => tracing::info!("Vulkan Video not supported; video will be coded in software"),
    }
    video
}

fn feature_request(
    instance: &VulkanInstance,
    candidate: &Candidate,
    request: DeviceRequest,
    transfer_queue: bool,
) -> FeatureRequest {
    let features = &candidate.features;
    let compute_missing = match request.compute {
        true => {
            FeatureRequest::computing().missing_on(instance.handle(), candidate.physical, features)
        }
        false => Vec::new(),
    };
    let wants = FeatureRequest {
        ycbcr: features.features11.sampler_ycbcr_conversion == vk::TRUE,
        three_d: request.three_d,
        compute: request.compute && compute_missing.is_empty(),
        anisotropy: features.features10.sampler_anisotropy == vk::TRUE,
        host_query_reset: transfer_queue && features.features12.host_query_reset == vk::TRUE,
        float16: features.features12.shader_float16 == vk::TRUE,
        int16: features.features10.shader_int16 == vk::TRUE,
    };
    if !compute_missing.is_empty() {
        tracing::warn!(
            "{} cannot run compute work, it lacks {}",
            candidate.name,
            compute_missing.join(", ")
        );
    }
    wants
}

fn create_device(
    instance: &VulkanInstance,
    candidate: &Candidate,
    request: DeviceRequest,
    queue_infos: &[vk::DeviceQueueCreateInfo<'_>],
    enabled: &EnabledExtensions,
    wants: FeatureRequest,
    video_maintenance: bool,
) -> Result<ash::Device> {
    let required = FeatureChain::required(wants);
    let mut features11 = required.features11;
    let mut features12 = required.features12;
    let mut features13 = required.features13;
    let mut video_maintenance1 =
        vk::PhysicalDeviceVideoMaintenance1FeaturesKHR::default().video_maintenance1(true);
    let mut memory_priority =
        vk::PhysicalDeviceMemoryPriorityFeaturesEXT::default().memory_priority(true);
    let mut pageable_memory = vk::PhysicalDevicePageableDeviceLocalMemoryFeaturesEXT::default()
        .pageable_device_local_memory(true);
    let mut structures =
        vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default().acceleration_structure(true);
    let mut ray_query = vk::PhysicalDeviceRayQueryFeaturesKHR::default().ray_query(true);
    let mut faults = vk::PhysicalDeviceFaultFeaturesEXT::default().device_fault(true);

    let mut device_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(queue_infos)
        .enabled_extension_names(&enabled.names)
        .enabled_features(&required.features10)
        .push_next(&mut features11)
        .push_next(&mut features12)
        .push_next(&mut features13);
    if video_maintenance {
        device_info = device_info.push_next(&mut video_maintenance1);
    }
    if enabled.memory.pageable {
        device_info = device_info
            .push_next(&mut memory_priority)
            .push_next(&mut pageable_memory);
    }
    if enabled.ray_tracing {
        device_info = device_info
            .push_next(&mut structures)
            .push_next(&mut ray_query);
    }
    if enabled.faults {
        device_info = device_info.push_next(&mut faults);
    }

    let create = |infos: &[vk::DeviceQueueCreateInfo<'_>], names: &[*const c_char]| unsafe {
        instance.handle().create_device(
            candidate.physical,
            &device_info
                .queue_create_infos(infos)
                .enabled_extension_names(names),
            None,
        )
    };
    let prioritized = request
        .prioritized
        .then(|| global_priority(&candidate.extensions))
        .flatten();
    let Some(priority_extension) = prioritized else {
        return Ok(create(queue_infos, &enabled.names)?);
    };
    let mut high = vk::DeviceQueueGlobalPriorityCreateInfoKHR::default()
        .global_priority(vk::QueueGlobalPriorityKHR::HIGH);
    let mut infos = queue_infos.to_vec();
    infos[0] = infos[0].push_next(&mut high);
    let mut names = enabled.names.clone();
    names.push(priority_extension.as_ptr());
    match create(&infos, &names) {
        Err(
            refused @ (vk::Result::ERROR_NOT_PERMITTED_KHR
            | vk::Result::ERROR_INITIALIZATION_FAILED),
        ) => {
            tracing::info!("high queue priority refused, using normal priority: {refused}");
            Ok(create(queue_infos, &enabled.names)?)
        }
        created => Ok(created?),
    }
}

#[cfg(unix)]
fn open_interop(instance: &VulkanInstance, device: &ash::Device, candidate: &Candidate) -> Interop {
    Interop {
        dmabuf: candidate
            .dmabuf
            .clone()
            .map(|support| open_dmabuf(instance, device, support)),
        drm_nodes: candidate.drm_nodes,
    }
}

#[cfg(windows)]
fn open_interop(instance: &VulkanInstance, device: &ash::Device, candidate: &Candidate) -> Interop {
    let win32 = match candidate.presentation {
        Some(presentation) => Some(Win32Device::new(instance, device, presentation)),
        None => {
            tracing::warn!("{} cannot present to windows", candidate.name);
            None
        }
    };
    Interop { win32 }
}

#[cfg(unix)]
fn open_dmabuf(
    instance: &VulkanInstance,
    device: &ash::Device,
    support: DmabufSupport,
) -> DmabufDevice {
    let semaphore_fd = support
        .sync_file()
        .then(|| khr::external_semaphore_fd::Device::new(instance.handle(), device));
    let dmabuf = DmabufDevice {
        memory_fd: khr::external_memory_fd::Device::new(instance.handle(), device),
        modifiers: ext::image_drm_format_modifier::Device::new(instance.handle(), device),
        semaphore_fd,
        support,
    };
    tracing::info!(
        export = dmabuf.support.sync_file_export,
        import = dmabuf.support.sync_file_import,
        "dmabuf import enabled"
    );
    dmabuf
}
