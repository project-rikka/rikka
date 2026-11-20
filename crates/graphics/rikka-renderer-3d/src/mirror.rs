use std::sync::Arc;

use ash::vk;
use bytemuck::Pod;

use crate::error::Result;
use rikka_gpu::core::barrier::MemoryAccess;
use rikka_gpu::core::buffer::GpuBuffer;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::memory::Placement;

struct Retiring {
    index: u32,
    after: u64,
}

pub(crate) struct MirroredTable<T, const WIDTH: usize = 1> {
    device: Arc<RenderDevice>,
    label: &'static str,
    readers: vk::PipelineStageFlags2,
    buffer: Option<GpuBuffer>,
    capacity: u32,
    values: Vec<T>,
    free: Vec<u32>,
    retiring: Vec<Retiring>,
    dirty: Option<(u32, u32)>,
}

impl<T: Pod, const WIDTH: usize> MirroredTable<T, WIDTH> {
    const ROW_BYTES: u64 = (WIDTH * size_of::<T>()) as u64;

    pub(crate) fn new(
        device: Arc<RenderDevice>,
        label: &'static str,
        readers: vk::PipelineStageFlags2,
    ) -> Self {
        Self {
            device,
            label,
            readers,
            buffer: None,
            capacity: 0,
            values: Vec::new(),
            free: Vec::new(),
            retiring: Vec::new(),
            dirty: None,
        }
    }

    pub(crate) fn with_capacity(
        device: Arc<RenderDevice>,
        label: &'static str,
        readers: vk::PipelineStageFlags2,
        capacity: u32,
    ) -> Result<Self> {
        let mut table = Self::new(device, label, readers);
        table.buffer = Some(table.allocate(capacity)?);
        table.capacity = capacity;
        Ok(table)
    }

    fn allocate(&self, capacity: u32) -> rikka_gpu::Result<GpuBuffer> {
        GpuBuffer::addressable(
            self.device.clone(),
            self.label,
            u64::from(capacity) * Self::ROW_BYTES,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            Placement::Device,
        )
    }

    pub(crate) fn address(&self) -> vk::DeviceAddress {
        self.buffer.as_ref().map_or(0, GpuBuffer::device_address)
    }

    fn rows(&self) -> u32 {
        (self.values.len() / WIDTH) as u32
    }

    pub(crate) fn insert(&mut self, fill: impl FnOnce(&mut [T; WIDTH])) -> u32 {
        let index = match self.free.pop() {
            Some(index) => index,
            None => {
                self.values.extend(std::iter::repeat_n(T::zeroed(), WIDTH));
                self.rows() - 1
            }
        };
        let (rows, _) = self.values.as_chunks_mut::<WIDTH>();
        fill(&mut rows[index as usize]);
        self.dirty = Some(match self.dirty {
            Some((first, last)) => (first.min(index), last.max(index)),
            None => (index, index),
        });
        index
    }

    pub(crate) fn retire(&mut self, index: u32, after: u64) {
        self.retiring.push(Retiring { index, after });
    }

    pub(crate) fn reclaim(&mut self, finished: u64) {
        let free = &mut self.free;
        self.retiring.retain(|retiring| {
            if retiring.after > finished {
                return true;
            }
            free.push(retiring.index);
            false
        });
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.dirty.is_some()
    }

    pub(crate) fn flush(&mut self, ctx: &mut GpuFrameCtx<'_>) -> Result<()> {
        self.reclaim(ctx.completed_value);
        let Some(dirty) = self.dirty else {
            return Ok(());
        };
        let rows = self.rows();
        let (first, last) = match rows > self.capacity {
            true => {
                let capacity = rows.next_power_of_two();
                let replacement = self.allocate(capacity)?;
                if let Some(previous) = self.buffer.replace(replacement) {
                    ctx.retire.defer(ctx.frame_value, Box::new(previous));
                }
                self.capacity = capacity;
                (0, rows - 1)
            }
            false => dirty,
        };
        self.dirty = None;
        let buffer = self
            .buffer
            .as_ref()
            .expect("a table holding rows has a buffer");
        let changed = &self.values[first as usize * WIDTH..(last as usize + 1) * WIDTH];
        let bytes: &[u8] = bytemuck::cast_slice(changed);
        let slice = ctx.staging.stage(bytes)?;
        let region = vk::BufferCopy {
            src_offset: slice.offset,
            dst_offset: u64::from(first) * Self::ROW_BYTES,
            size: bytes.len() as vk::DeviceSize,
        };
        ctx.memory_barrier(
            MemoryAccess::new(self.readers, vk::AccessFlags2::NONE),
            MemoryAccess::new(
                vk::PipelineStageFlags2::COPY,
                vk::AccessFlags2::TRANSFER_WRITE,
            ),
        );
        unsafe {
            ctx.device
                .handle()
                .cmd_copy_buffer(ctx.cmd, slice.buffer, buffer.handle(), &[region]);
        }
        Ok(())
    }
}
