use std::mem::ManuallyDrop;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ash::vk;

use crate::core::device::RenderDevice;
use crate::core::memory::{Allocation, Placement};
use crate::error::Result;

static SERIALS: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferBinding {
    pub handle: vk::Buffer,
    serial: u64,
}

pub struct GpuBuffer {
    device: Arc<RenderDevice>,
    buffer: vk::Buffer,
    allocation: ManuallyDrop<Allocation>,
    size: vk::DeviceSize,
    serial: u64,
}

impl GpuBuffer {
    pub fn new(
        device: Arc<RenderDevice>,
        name: &str,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        location: Placement,
    ) -> Result<Self> {
        Self::create(device, name, size, usage, location, None)
    }

    pub(crate) fn staging(
        device: Arc<RenderDevice>,
        name: &str,
        size: vk::DeviceSize,
    ) -> Result<Self> {
        Self::create(
            device,
            name,
            size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            Placement::Staging,
            None,
        )
    }

    pub(crate) fn create(
        device: Arc<RenderDevice>,
        name: &str,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        placement: Placement,
        shared_by: Option<[u32; 2]>,
    ) -> Result<Self> {
        let families = shared_by.unwrap_or_default();
        let info = vk::BufferCreateInfo::default().size(size).usage(usage);
        let info = match shared_by {
            Some(_) => info
                .sharing_mode(vk::SharingMode::CONCURRENT)
                .queue_family_indices(&families),
            None => info.sharing_mode(vk::SharingMode::EXCLUSIVE),
        };
        let buffer = unsafe { device.handle().create_buffer(&info, None)? };
        let allocation = match device.allocate_buffer(name, buffer, placement) {
            Ok(allocation) => allocation,
            Err(err) => {
                unsafe { device.handle().destroy_buffer(buffer, None) };
                return Err(err);
            }
        };
        let bound = unsafe {
            device
                .handle()
                .bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
        };
        if let Err(err) = bound {
            unsafe { device.handle().destroy_buffer(buffer, None) };
            device.free(allocation);
            return Err(err.into());
        }

        device.set_object_name(buffer, name);

        Ok(Self {
            device,
            buffer,
            allocation: ManuallyDrop::new(allocation),
            size,
            serial: SERIALS.fetch_add(1, Ordering::Relaxed),
        })
    }

    pub fn handle(&self) -> vk::Buffer {
        self.buffer
    }

    pub fn binding(&self) -> BufferBinding {
        BufferBinding {
            handle: self.buffer,
            serial: self.serial,
        }
    }

    pub fn size(&self) -> vk::DeviceSize {
        self.size
    }

    pub fn mapped_slice(&self) -> Option<&[u8]> {
        self.allocation.mapped_slice()
    }

    pub fn mapped_slice_mut(&mut self) -> Option<&mut [u8]> {
        self.allocation.mapped_slice_mut()
    }

    pub fn device_address(&self) -> vk::DeviceAddress {
        let info = vk::BufferDeviceAddressInfo::default().buffer(self.buffer);
        unsafe { self.device.handle().get_buffer_device_address(&info) }
    }
    pub fn addressable(
        device: Arc<RenderDevice>,
        name: &str,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        location: Placement,
    ) -> Result<Self> {
        Self::new(
            device,
            name,
            size,
            usage | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            location,
        )
    }
}

impl Drop for GpuBuffer {
    fn drop(&mut self) {
        unsafe {
            self.device.handle().destroy_buffer(self.buffer, None);
            let allocation = ManuallyDrop::take(&mut self.allocation);
            self.device.free(allocation);
        }
    }
}

impl std::fmt::Debug for GpuBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuBuffer").finish_non_exhaustive()
    }
}
