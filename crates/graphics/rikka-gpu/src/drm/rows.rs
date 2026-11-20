use std::os::fd::{FromRawFd, OwnedFd};
use std::sync::Arc;

use ash::vk;

use crate::core::device::RenderDevice;
use crate::drm::export::{Placement, memory_type};
use crate::error::{Error, Result};
use crate::reclaim::Deferred;

const ROW_ALIGNMENT: u32 = 256;

pub struct RowsMemory {
    device: Arc<RenderDevice>,
    pub buffer: vk::Buffer,
    memory: vk::DeviceMemory,
}

pub struct SharedRows {
    pub fd: OwnedFd,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub memory: Deferred<RowsMemory>,
}

impl SharedRows {
    pub const FOURCC: u32 = u32::from_le_bytes(*b"AB24");
}

pub fn export_rows(
    device: Arc<RenderDevice>,
    width: u32,
    height: u32,
) -> Result<(RowsMemory, OwnedFd, u32)> {
    let dmabuf = device
        .dmabuf()
        .ok_or_else(|| Error::Unsupported("device does not support dmabuf sharing".into()))?;
    let stride = (width * 4).div_ceil(ROW_ALIGNMENT) * ROW_ALIGNMENT;
    let size = u64::from(stride) * u64::from(height);
    let handle = vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT;
    let usage = vk::BufferUsageFlags::TRANSFER_DST;
    let mut properties = vk::ExternalBufferProperties::default();
    let query = vk::PhysicalDeviceExternalBufferInfo::default()
        .usage(usage)
        .handle_type(handle);
    unsafe {
        device
            .instance()
            .handle()
            .get_physical_device_external_buffer_properties(
                device.physical(),
                &query,
                &mut properties,
            );
    }
    if !properties
        .external_memory_properties
        .external_memory_features
        .contains(vk::ExternalMemoryFeatureFlags::EXPORTABLE)
    {
        return Err(Error::Unsupported(
            "device does not support dmabuf export of buffer memory".into(),
        ));
    }
    let mut external = vk::ExternalMemoryBufferCreateInfo::default().handle_types(handle);
    let info = vk::BufferCreateInfo::default()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .push_next(&mut external);
    let buffer = unsafe { device.handle().create_buffer(&info, None)? };
    let memory = match allocate(&device, buffer) {
        Ok(memory) => memory,
        Err(error) => {
            unsafe { device.handle().destroy_buffer(buffer, None) };
            return Err(error);
        }
    };
    let rows = RowsMemory {
        device: device.clone(),
        buffer,
        memory,
    };
    unsafe { device.handle().bind_buffer_memory(buffer, memory, 0)? };
    device.set_object_name(buffer, "rikka.rows.shared");
    let fd_info = vk::MemoryGetFdInfoKHR::default()
        .memory(memory)
        .handle_type(handle);
    let fd = unsafe { dmabuf.memory_fd.get_memory_fd(&fd_info)? };
    Ok((rows, unsafe { OwnedFd::from_raw_fd(fd) }, stride))
}

fn allocate(device: &RenderDevice, buffer: vk::Buffer) -> Result<vk::DeviceMemory> {
    let requirements = unsafe { device.handle().get_buffer_memory_requirements(buffer) };
    let properties = unsafe {
        device
            .instance()
            .handle()
            .get_physical_device_memory_properties(device.physical())
    };
    let memory_type = memory_type(
        &properties,
        requirements.memory_type_bits,
        Placement::System,
    )
    .ok_or_else(|| Error::Unsupported("no memory type supports shared rows".into()))?;
    let mut export = vk::ExportMemoryAllocateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().buffer(buffer);
    let allocate = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memory_type)
        .push_next(&mut export)
        .push_next(&mut dedicated);
    Ok(unsafe { device.handle().allocate_memory(&allocate, None)? })
}

impl Drop for RowsMemory {
    fn drop(&mut self) {
        unsafe {
            self.device.handle().destroy_buffer(self.buffer, None);
            self.device.handle().free_memory(self.memory, None);
        }
    }
}

impl std::fmt::Debug for SharedRows {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedRows")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride", &self.stride)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for RowsMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RowsMemory").finish_non_exhaustive()
    }
}
