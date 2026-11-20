use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::time::{Duration, Instant};

use ash::{ext, vk};
use gpu_allocator::vulkan::{
    AllocationCreateDesc, AllocationScheme, Allocator, AllocatorCreateDesc,
};
use gpu_allocator::{AllocationError, AllocationSizes, MemoryLocation};
use rikka_sync::Mutex;

use crate::error::{Error, Result};

pub const MEMORY_TARGET: &str = "rikka_renderer::memory";

pub(crate) const MB: u64 = 1024 * 1024;

const ATTACHMENT_PRIORITY: f32 = 1.0;

const LARGE_TARGET: u64 = MB;

const BUDGET_EVERY: Duration = Duration::from_secs(5);

const PRESSURE_THRESHOLD: f64 = 0.9;

fn allocation_sizes() -> AllocationSizes {
    AllocationSizes::new(4 * MB, 4 * MB)
        .with_max_device_memblock_size(32 * MB)
        .with_max_host_memblock_size(16 * MB)
}
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MemoryFeatures {
    pub budget: bool,
    pub pageable: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct HeapBudget {
    pub heap: u32,
    pub device_local: bool,
    pub usage: u64,
    pub budget: u64,
}

impl HeapBudget {
    fn under_pressure(&self) -> bool {
        self.budget > 0 && self.usage as f64 >= self.budget as f64 * PRESSURE_THRESHOLD
    }
}

#[derive(Default)]
struct Kind {
    bytes: u64,
    count: u32,
    largest: u64,
}

#[derive(Default)]
struct BudgetWatch {
    checked: Option<Instant>,
    under_pressure: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Device,
    Upload,
    Readback,
    Staging,
}

impl Placement {
    fn location(self) -> MemoryLocation {
        match self {
            Self::Device => MemoryLocation::GpuOnly,
            Self::Upload | Self::Staging => MemoryLocation::CpuToGpu,
            Self::Readback => MemoryLocation::GpuToCpu,
        }
    }
}

pub struct Allocation(gpu_allocator::vulkan::Allocation);

impl Allocation {
    pub fn memory(&self) -> vk::DeviceMemory {
        unsafe { self.0.memory() }
    }

    pub fn offset(&self) -> vk::DeviceSize {
        self.0.offset()
    }

    pub(crate) fn mapped_slice(&self) -> Option<&[u8]> {
        self.0.mapped_slice()
    }

    pub(crate) fn mapped_slice_mut(&mut self) -> Option<&mut [u8]> {
        self.0.mapped_slice_mut()
    }
}

impl From<AllocationError> for Error {
    fn from(source: AllocationError) -> Self {
        Self::Allocation(Box::new(source))
    }
}

pub(crate) struct DeviceMemory {
    instance: ash::Instance,
    physical: vk::PhysicalDevice,
    device: ash::Device,
    allocator: Mutex<ManuallyDrop<Allocator>>,
    pageable: Option<ext::pageable_device_local_memory::Device>,
    reports_budget: bool,
    host_types: u32,
    staging_types: u32,
    lazy_types: u32,
    watch: Mutex<BudgetWatch>,
}

impl DeviceMemory {
    pub(crate) fn new(
        instance: &ash::Instance,
        physical: vk::PhysicalDevice,
        device: &ash::Device,
        features: MemoryFeatures,
        buffer_device_address: bool,
    ) -> Result<Self> {
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.clone(),
            physical_device: physical,
            debug_settings: Default::default(),
            buffer_device_address,
            allocation_sizes: allocation_sizes(),
        })?;

        let properties = unsafe { instance.get_physical_device_memory_properties(physical) };

        Ok(Self {
            instance: instance.clone(),
            physical,
            device: device.clone(),
            allocator: Mutex::new(ManuallyDrop::new(allocator)),
            pageable: features
                .pageable
                .then(|| ext::pageable_device_local_memory::Device::new(instance, device)),
            reports_budget: features.budget,
            host_types: memory_types_with(
                &properties,
                vk::MemoryPropertyFlags::empty(),
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            ),
            staging_types: memory_types_with(
                &properties,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            ),
            lazy_types: memory_types_with(
                &properties,
                vk::MemoryPropertyFlags::LAZILY_ALLOCATED,
                vk::MemoryPropertyFlags::empty(),
            ),
            watch: Mutex::new(BudgetWatch::default()),
        })
    }

    pub(crate) fn allocate_image(
        &self,
        name: &str,
        image: vk::Image,
        usage: vk::ImageUsageFlags,
    ) -> Result<Allocation> {
        let info = vk::ImageMemoryRequirementsInfo2::default().image(image);
        let mut dedicated = vk::MemoryDedicatedRequirements::default();
        let requirements = {
            let mut requirements = vk::MemoryRequirements2::default().push_next(&mut dedicated);
            unsafe {
                self.device
                    .get_image_memory_requirements2(&info, &mut requirements)
            };
            requirements.memory_requirements
        };
        if usage.contains(vk::ImageUsageFlags::TRANSIENT_ATTACHMENT)
            && let Some(allocation) = self.allocate_lazily(name, image, requirements)
        {
            return Ok(allocation);
        }
        let attachment = usage.intersects(
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
        );
        let large_target = attachment && requirements.size >= LARGE_TARGET;
        let scheme = match large_target || prefers_dedicated(&dedicated) {
            true => AllocationScheme::DedicatedImage(image),
            false => AllocationScheme::GpuAllocatorManaged,
        };
        let allocation =
            self.allocate(name, requirements, MemoryLocation::GpuOnly, false, scheme)?;
        if attachment {
            self.raise_priority(&allocation);
        }
        Ok(allocation)
    }

    pub(crate) fn allocate_heap(
        &self,
        name: &str,
        requirements: vk::MemoryRequirements,
    ) -> Result<Allocation> {
        self.allocate(
            name,
            requirements,
            MemoryLocation::GpuOnly,
            false,
            AllocationScheme::GpuAllocatorManaged,
        )
    }

    fn allocate_lazily(
        &self,
        name: &str,
        image: vk::Image,
        requirements: vk::MemoryRequirements,
    ) -> Option<Allocation> {
        let lazy_types = requirements.memory_type_bits & self.lazy_types;
        if lazy_types == 0 {
            return None;
        }
        let desc = AllocationCreateDesc {
            name,
            requirements: vk::MemoryRequirements {
                memory_type_bits: lazy_types,
                ..requirements
            },
            location: MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: AllocationScheme::DedicatedImage(image),
        };
        self.allocator
            .lock()
            .allocate(&desc)
            .inspect_err(|error| {
                tracing::debug!(name, "lazily allocated memory unavailable: {error}")
            })
            .ok()
            .map(Allocation)
    }

    pub(crate) fn allocate_buffer(
        &self,
        name: &str,
        buffer: vk::Buffer,
        placement: Placement,
    ) -> Result<Allocation> {
        let info = vk::BufferMemoryRequirementsInfo2::default().buffer(buffer);
        let mut dedicated = vk::MemoryDedicatedRequirements::default();
        let mut requirements = {
            let mut requirements = vk::MemoryRequirements2::default().push_next(&mut dedicated);
            unsafe {
                self.device
                    .get_buffer_memory_requirements2(&info, &mut requirements)
            };
            requirements.memory_requirements
        };
        if placement == Placement::Staging
            && requirements.memory_type_bits & self.staging_types != 0
        {
            requirements.memory_type_bits &= self.staging_types;
        }
        let scheme = match prefers_dedicated(&dedicated) {
            true => AllocationScheme::DedicatedBuffer(buffer),
            false => AllocationScheme::GpuAllocatorManaged,
        };
        self.allocate(name, requirements, placement.location(), true, scheme)
    }

    fn allocate(
        &self,
        name: &str,
        requirements: vk::MemoryRequirements,
        location: MemoryLocation,
        linear: bool,
        allocation_scheme: AllocationScheme,
    ) -> Result<Allocation> {
        self.check_budget();
        let desc = AllocationCreateDesc {
            name,
            requirements,
            location,
            linear,
            allocation_scheme,
        };
        let mut allocator = self.allocator.lock();
        let allocated = match allocator.allocate(&desc) {
            Err(AllocationError::OutOfMemory) if location == MemoryLocation::GpuOnly => {
                self.report_exhaustion(name);
                let host_types = requirements.memory_type_bits & self.host_types;
                if host_types == 0 {
                    return Err(AllocationError::OutOfMemory.into());
                }
                tracing::warn!(
                    name,
                    "video memory exhausted, falling back to system memory"
                );
                allocator.allocate(&AllocationCreateDesc {
                    requirements: vk::MemoryRequirements {
                        memory_type_bits: host_types,
                        ..requirements
                    },
                    location: MemoryLocation::Unknown,
                    ..desc
                })
            }
            allocated => allocated,
        };
        Ok(Allocation(allocated?))
    }

    fn raise_priority(&self, allocation: &Allocation) {
        let Some(pageable) = &self.pageable else {
            return;
        };
        if !allocation.0.is_dedicated() {
            return;
        }
        unsafe {
            (pageable.fp().set_device_memory_priority_ext)(
                pageable.device(),
                allocation.memory(),
                ATTACHMENT_PRIORITY,
            )
        };
    }

    pub(crate) fn free(&self, allocation: Allocation) {
        let mut allocator = self.allocator.lock();
        if let Err(err) = allocator.free(allocation.0) {
            tracing::error!("failed to free GPU allocation: {err}");
        }
    }

    pub(crate) fn budget(&self) -> Option<Vec<HeapBudget>> {
        if !self.reports_budget {
            return None;
        }
        let mut budget = vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
        let memory = {
            let mut properties =
                vk::PhysicalDeviceMemoryProperties2::default().push_next(&mut budget);
            unsafe {
                self.instance
                    .get_physical_device_memory_properties2(self.physical, &mut properties)
            };
            properties.memory_properties
        };
        Some(
            memory
                .memory_heaps_as_slice()
                .iter()
                .enumerate()
                .map(|(index, heap)| HeapBudget {
                    heap: index as u32,
                    device_local: heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL),
                    usage: budget.heap_usage[index],
                    budget: budget.heap_budget[index],
                })
                .collect(),
        )
    }

    fn check_budget(&self) {
        let now = Instant::now();
        let mut watch = self.watch.lock();
        if watch
            .checked
            .is_some_and(|checked| now.duration_since(checked) < BUDGET_EVERY)
        {
            return;
        }
        watch.checked = Some(now);
        self.report_usage();
        let Some(heaps) = self.budget() else { return };
        let under_pressure: Vec<&HeapBudget> = heaps
            .iter()
            .filter(|heap| heap.device_local && heap.under_pressure())
            .collect();
        match (under_pressure.is_empty(), watch.under_pressure) {
            (false, false) => {
                for heap in under_pressure {
                    tracing::warn!(
                        heap = heap.heap,
                        used_mb = heap.usage / MB,
                        budget_mb = heap.budget / MB,
                        "video memory is nearly exhausted"
                    );
                }
                watch.under_pressure = true;
            }
            (true, true) => watch.under_pressure = false,
            _ => {}
        }
    }

    fn report_exhaustion(&self, name: &str) {
        match self.budget() {
            Some(heaps) => {
                for heap in heaps.iter().filter(|heap| heap.device_local) {
                    tracing::warn!(
                        name,
                        heap = heap.heap,
                        used_mb = heap.usage / MB,
                        budget_mb = heap.budget / MB,
                        "video memory allocation failed"
                    );
                }
            }
            None => tracing::warn!(name, "video memory allocation failed"),
        }
    }

    pub(crate) fn report_usage(&self) {
        if !tracing::enabled!(target: MEMORY_TARGET, tracing::Level::DEBUG) {
            return;
        }
        let report = self.allocator.lock().generate_report();
        let mut by_name: HashMap<String, Kind> = HashMap::new();
        for allocation in &report.allocations {
            let kind: String = allocation
                .name
                .chars()
                .filter(|c| !c.is_ascii_digit())
                .collect();
            let entry = by_name.entry(kind).or_default();
            entry.bytes += allocation.size;
            entry.count += 1;
            entry.largest = entry.largest.max(allocation.size);
        }
        let mut kinds: Vec<(String, Kind)> = by_name.into_iter().collect();
        kinds.sort_by_key(|(_, kind)| std::cmp::Reverse(kind.bytes));
        tracing::debug!(
            target: MEMORY_TARGET,
            allocated_kb = report.total_allocated_bytes / 1024,
            reserved_kb = report.total_capacity_bytes / 1024,
            blocks = report.blocks.len(),
            "gpu memory"
        );
        for (kind, usage) in kinds.into_iter().take(24) {
            tracing::debug!(
                target: MEMORY_TARGET,
                kind,
                kb = usage.bytes / 1024,
                count = usage.count,
                largest_kb = usage.largest / 1024,
                "gpu memory kind"
            );
        }
    }

    pub(crate) unsafe fn close(&self) {
        let mut allocator = self.allocator.lock();
        unsafe { ManuallyDrop::drop(&mut allocator) };
    }
}

fn prefers_dedicated(dedicated: &vk::MemoryDedicatedRequirements<'_>) -> bool {
    dedicated.prefers_dedicated_allocation == vk::TRUE
        || dedicated.requires_dedicated_allocation == vk::TRUE
}

fn memory_types_with(
    properties: &vk::PhysicalDeviceMemoryProperties,
    with: vk::MemoryPropertyFlags,
    without: vk::MemoryPropertyFlags,
) -> u32 {
    properties
        .memory_types_as_slice()
        .iter()
        .enumerate()
        .filter(|(_, memory)| {
            memory.property_flags.contains(with) && !memory.property_flags.intersects(without)
        })
        .fold(0u32, |bits, (index, _)| bits | (1 << index))
}

impl std::fmt::Debug for Allocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Allocation").finish_non_exhaustive()
    }
}
