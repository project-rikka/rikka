use std::sync::Arc;

use ash::vk;

use crate::error::Result;
use rikka_gpu::core::buffer::GpuBuffer;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::memory::Placement;

pub(crate) struct Structure {
    device: Arc<RenderDevice>,
    buffer: GpuBuffer,
    pub handle: vk::AccelerationStructureKHR,
    pub address: vk::DeviceAddress,
}

impl Structure {
    pub(crate) fn new(
        device: &Arc<RenderDevice>,
        name: &str,
        ty: vk::AccelerationStructureTypeKHR,
        size: vk::DeviceSize,
    ) -> Result<Self> {
        let rays = device
            .ray_tracing()
            .expect("acceleration structures exist only with ray tracing");
        let buffer = GpuBuffer::addressable(
            device.clone(),
            name,
            size,
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR,
            Placement::Device,
        )?;
        let info = vk::AccelerationStructureCreateInfoKHR::default()
            .buffer(buffer.handle())
            .size(size)
            .ty(ty);
        let handle = unsafe { rays.structures.create_acceleration_structure(&info, None)? };
        device.set_object_name(handle, name);
        let address = unsafe {
            rays.structures.get_acceleration_structure_device_address(
                &vk::AccelerationStructureDeviceAddressInfoKHR::default()
                    .acceleration_structure(handle),
            )
        };
        Ok(Self {
            device: device.clone(),
            buffer,
            handle,
            address,
        })
    }

    pub(crate) fn size(&self) -> vk::DeviceSize {
        self.buffer.size()
    }
}

impl Drop for Structure {
    fn drop(&mut self) {
        if let Some(rays) = self.device.ray_tracing() {
            unsafe {
                rays.structures
                    .destroy_acceleration_structure(self.handle, None)
            };
        }
    }
}
