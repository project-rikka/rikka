use std::sync::Arc;

use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::error::Result;
use crate::look::gpu::split;
use crate::pool::traced_input;
use crate::shader::MaterialShader;
use crate::vertex::{GpuAttribute, GpuPosition};
use rikka_gpu::core::buffer::GpuBuffer;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::memory::Placement;

const MIN_VERTICES: u32 = 4096;
const MIN_JOBS: u32 = 16;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
struct GpuHookJob {
    instance: u32,
    vertex_count: u32,
    positions: [u32; 2],
    attributes: [u32; 2],
}

pub(crate) struct HookJob {
    pub shader: MaterialShader,
    pub instance: u32,
    pub vertex_count: u32,
    pub at: u32,
}

struct Table {
    buffer: GpuBuffer,
    capacity: u32,
}

struct Outputs {
    positions: GpuBuffer,
    attributes: GpuBuffer,
}

pub(super) struct Hooks {
    device: Arc<RenderDevice>,
    jobs: Vec<HookJob>,
    table: Vec<GpuHookJob>,
    vertices: u32,
    outputs: Option<Outputs>,
    capacity: u32,
    tables: Vec<Table>,
}

impl Hooks {
    pub(super) fn new(device: Arc<RenderDevice>) -> Self {
        Self {
            device,
            jobs: Vec::new(),
            table: Vec::new(),
            vertices: 0,
            outputs: None,
            capacity: 0,
            tables: Vec::new(),
        }
    }

    pub(super) fn clear(&mut self) {
        self.jobs.clear();
        self.vertices = 0;
    }

    pub(super) fn push(
        &mut self,
        shader: &MaterialShader,
        instance: u32,
        vertex_count: u32,
    ) -> u32 {
        let at = self.vertices;
        self.vertices += vertex_count;
        self.jobs.push(HookJob {
            shader: shader.clone(),
            instance,
            vertex_count,
            at,
        });
        self.jobs.len() as u32 - 1
    }

    pub(super) fn jobs(&self) -> &[HookJob] {
        &self.jobs
    }

    pub(super) fn ensure(&mut self, ctx: &mut GpuFrameCtx<'_>) -> Result<()> {
        if self.vertices <= self.capacity && self.outputs.is_some() {
            return Ok(());
        }
        let capacity = self.vertices.next_power_of_two().max(MIN_VERTICES);
        let replacement = Outputs {
            positions: GpuBuffer::addressable(
                self.device.clone(),
                "rikka.ray.hooked",
                u64::from(capacity) * size_of::<GpuPosition>() as u64,
                vk::BufferUsageFlags::STORAGE_BUFFER | traced_input(&self.device),
                Placement::Device,
            )?,
            attributes: GpuBuffer::addressable(
                self.device.clone(),
                "rikka.ray.hooked_attributes",
                u64::from(capacity) * size_of::<GpuAttribute>() as u64,
                vk::BufferUsageFlags::STORAGE_BUFFER,
                Placement::Device,
            )?,
        };
        if let Some(former) = self.outputs.replace(replacement) {
            ctx.retire
                .defer(ctx.frame_value, Box::new(former.positions));
            ctx.retire
                .defer(ctx.frame_value, Box::new(former.attributes));
        }
        self.capacity = capacity;
        Ok(())
    }

    pub(super) fn positions(&self, job: u32) -> vk::DeviceAddress {
        self.address(job, |outputs| &outputs.positions, size_of::<GpuPosition>())
    }

    fn attributes(&self, job: u32) -> vk::DeviceAddress {
        self.address(
            job,
            |outputs| &outputs.attributes,
            size_of::<GpuAttribute>(),
        )
    }

    fn address(
        &self,
        job: u32,
        buffer: impl Fn(&Outputs) -> &GpuBuffer,
        stride: usize,
    ) -> vk::DeviceAddress {
        let base = self
            .outputs
            .as_ref()
            .map_or(0, |outputs| buffer(outputs).device_address());
        base + u64::from(self.jobs[job as usize].at) * stride as u64
    }

    pub(super) fn write_table(&mut self, ctx: &mut GpuFrameCtx<'_>) -> Result<vk::DeviceAddress> {
        if self.jobs.is_empty() {
            return Ok(0);
        }
        self.table.clear();
        for (job, hook) in self.jobs.iter().enumerate() {
            self.table.push(GpuHookJob {
                instance: hook.instance,
                vertex_count: hook.vertex_count,
                positions: split(self.positions(job as u32)),
                attributes: split(self.attributes(job as u32)),
            });
        }
        while self.tables.len() <= ctx.frame_slot {
            self.tables.push(Table {
                buffer: table_buffer(&self.device, MIN_JOBS)?,
                capacity: MIN_JOBS,
            });
        }
        let table = &mut self.tables[ctx.frame_slot];
        let count = self.table.len() as u32;
        if count > table.capacity {
            let capacity = count.next_power_of_two();
            let former =
                std::mem::replace(&mut table.buffer, table_buffer(&self.device, capacity)?);
            ctx.retire.defer(ctx.frame_value, Box::new(former));
            table.capacity = capacity;
        }
        let bytes: &[u8] = bytemuck::cast_slice(&self.table);
        table
            .buffer
            .mapped_slice_mut()
            .expect("hook jobs are host-visible")[..bytes.len()]
            .copy_from_slice(bytes);
        Ok(table.buffer.device_address())
    }
}

fn table_buffer(device: &Arc<RenderDevice>, capacity: u32) -> rikka_gpu::Result<GpuBuffer> {
    GpuBuffer::addressable(
        device.clone(),
        "rikka.ray.hook_jobs",
        u64::from(capacity) * size_of::<GpuHookJob>() as u64,
        vk::BufferUsageFlags::STORAGE_BUFFER,
        Placement::Upload,
    )
}
