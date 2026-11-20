use std::sync::Arc;

use ash::vk;

use crate::core::buffer::GpuBuffer;
use crate::core::device::RenderDevice;
use crate::core::frame::MAX_FRAMES_IN_FLIGHT;
use crate::core::memory::Placement;
use crate::error::{Error, Result, Store};

pub(crate) const PLACED_PER_FRAME: u32 = 64;

pub const PLACED_SLOTS: u32 = PLACED_PER_FRAME * MAX_FRAMES_IN_FLIGHT as u32;

const EXTENT_BYTES: usize = size_of::<[u32; 2]>();

pub(crate) struct PlacedSlots {
    extents: GpuBuffer,
    frame: u32,
    used: u32,
}

impl PlacedSlots {
    pub(crate) fn new(device: Arc<RenderDevice>) -> Result<Self> {
        let mut extents = GpuBuffer::new(
            device,
            "rikka.placed_extents",
            (PLACED_SLOTS as usize * EXTENT_BYTES) as vk::DeviceSize,
            vk::BufferUsageFlags::STORAGE_BUFFER,
            Placement::Upload,
        )?;
        extents
            .mapped_slice_mut()
            .expect("placed extents are host mapped")
            .fill(0);
        Ok(Self {
            extents,
            frame: 0,
            used: 0,
        })
    }

    pub(crate) fn buffer(&self) -> vk::Buffer {
        self.extents.handle()
    }

    pub(crate) fn begin(&mut self, frame_slot: usize) {
        self.frame = frame_slot as u32;
        self.used = 0;
    }

    pub(crate) fn take(&mut self, extent: vk::Extent2D) -> Result<u32> {
        if self.used == PLACED_PER_FRAME {
            return Err(Error::Full(Store::Table {
                what: "placed image",
                slots: PLACED_PER_FRAME,
            }));
        }
        let slot = self.frame * PLACED_PER_FRAME + self.used;
        self.used += 1;
        let at = slot as usize * EXTENT_BYTES;
        self.extents
            .mapped_slice_mut()
            .expect("placed extents are host mapped")[at..at + EXTENT_BYTES]
            .copy_from_slice(bytemuck::bytes_of(&[extent.width, extent.height]));
        Ok(slot)
    }
}
