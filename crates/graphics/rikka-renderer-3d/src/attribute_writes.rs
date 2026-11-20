use std::sync::Arc;

use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use half::f16;

use crate::arena::drawn_before;
use crate::error::Result;
use crate::mesh::MeshUpdate;
use crate::pipeline::MeshPipelines;
use crate::record::command::{bind_compute, dispatch, push_constants};
use crate::vertex::{GpuAttribute, encode_octahedral, encode_tangent};
use rikka_gpu::core::barrier::MemoryAccess;
use rikka_gpu::core::buffer::GpuBuffer;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::memory::Placement;

const NORMAL: u32 = 1 << 0;
const TANGENT: u32 = 1 << 1;
const UV: u32 = 1 << 2;
const COLOR: u32 = 1 << 3;

const GROUP_SIZE: u32 = 64;
const MAX_GROUPS: u32 = 65535;
const CHUNK: u32 = GROUP_SIZE * MAX_GROUPS;
const MIN_SCRATCH: u64 = 64 * 1024;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WritePush {
    target: u64,
    source: u64,
    count: u32,
    fields: u32,
}

#[derive(Clone, Copy)]
struct Write {
    target: u32,
    count: u32,
    fields: u32,
    source: u32,
}

pub(crate) struct AttributeWrites {
    device: Arc<RenderDevice>,
    writes: Vec<Write>,
    words: Vec<u32>,
    scratch: Option<GpuBuffer>,
}

impl AttributeWrites {
    pub(crate) fn new(device: Arc<RenderDevice>) -> Self {
        Self {
            device,
            writes: Vec::new(),
            words: Vec::new(),
            scratch: None,
        }
    }

    pub(crate) fn push(&mut self, target: u32, update: &MeshUpdate<'_>, count: u32) {
        let fields = [
            (!update.normals.is_empty(), NORMAL),
            (!update.tangents.is_empty(), TANGENT),
            (!update.uvs.is_empty(), UV),
            (!update.colors.is_empty(), COLOR),
        ]
        .into_iter()
        .filter_map(|(given, field)| given.then_some(field))
        .fold(0, |fields, field| fields | field);
        if fields == 0 {
            return;
        }
        let mut first = 0;
        while first < count {
            let length = (count - first).min(CHUNK);
            let range = first as usize..(first + length) as usize;
            let source = self.words.len() as u32;
            if fields & NORMAL != 0 {
                self.words.extend(
                    update.normals[range.clone()]
                        .iter()
                        .map(|&n| encode_octahedral(n)),
                );
            }
            if fields & TANGENT != 0 {
                self.words.extend(
                    update.tangents[range.clone()]
                        .iter()
                        .map(|&t| encode_tangent(t)),
                );
            }
            if fields & UV != 0 {
                self.words
                    .extend(update.uvs[range.clone()].iter().map(|&uv| packed_uv(uv)));
            }
            if fields & COLOR != 0 {
                self.words.extend(
                    update.colors[range]
                        .iter()
                        .map(|&color| u32::from_le_bytes(color)),
                );
            }
            self.writes.push(Write {
                target: target + first,
                count: length,
                fields,
                source,
            });
            first += length;
        }
    }

    pub(crate) fn has_pending(&self) -> bool {
        !self.writes.is_empty()
    }

    pub(crate) fn flush(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        pipelines: &mut MeshPipelines,
        attributes: vk::DeviceAddress,
    ) -> Result<()> {
        if self.writes.is_empty() {
            return Ok(());
        }
        let scratch = self.scratch(ctx, self.words.len() as u64 * 4)?;
        let (scratch_buffer, scratch_address) = (scratch.handle(), scratch.device_address());
        let bytes: &[u8] = bytemuck::cast_slice(&self.words);
        let writes_scratch = MemoryAccess::new(
            vk::PipelineStageFlags2::COPY | vk::PipelineStageFlags2::COMPUTE_SHADER,
            vk::AccessFlags2::TRANSFER_WRITE | vk::AccessFlags2::SHADER_STORAGE_WRITE,
        );
        ctx.memory_barrier(drawn_before(), writes_scratch);
        let slice = ctx.staging.stage(bytes)?;
        let region = vk::BufferCopy {
            src_offset: slice.offset,
            dst_offset: 0,
            size: bytes.len() as vk::DeviceSize,
        };
        unsafe {
            ctx.device
                .handle()
                .cmd_copy_buffer(ctx.cmd, slice.buffer, scratch_buffer, &[region]);
        }
        ctx.memory_barrier(
            MemoryAccess::new(
                vk::PipelineStageFlags2::COPY,
                vk::AccessFlags2::TRANSFER_WRITE,
            ),
            MemoryAccess::new(
                vk::PipelineStageFlags2::COMPUTE_SHADER,
                vk::AccessFlags2::SHADER_STORAGE_READ,
            ),
        );
        let pipeline = pipelines.kernel(shaders_3d::MESH_ATTRIBUTES_COMP)?;
        bind_compute(ctx, pipeline);
        for write in self.writes.drain(..) {
            let push = WritePush {
                target: attributes + u64::from(write.target) * size_of::<GpuAttribute>() as u64,
                source: scratch_address + u64::from(write.source) * 4,
                count: write.count,
                fields: write.fields,
            };
            push_constants(ctx, pipelines.layout(), bytemuck::bytes_of(&push));
            dispatch(ctx, write.count.div_ceil(GROUP_SIZE), 1, 1);
        }
        self.words.clear();
        ctx.memory_barrier(
            MemoryAccess::new(
                vk::PipelineStageFlags2::COMPUTE_SHADER,
                vk::AccessFlags2::SHADER_STORAGE_WRITE,
            ),
            MemoryAccess::new(
                vk::PipelineStageFlags2::ALL_COMMANDS,
                vk::AccessFlags2::MEMORY_READ,
            ),
        );
        Ok(())
    }

    fn scratch(&mut self, ctx: &mut GpuFrameCtx<'_>, needed: u64) -> Result<&GpuBuffer> {
        let fits = self
            .scratch
            .as_ref()
            .is_some_and(|scratch| scratch.size() >= needed);
        if !fits {
            let replacement = GpuBuffer::addressable(
                self.device.clone(),
                "rikka.three.attribute_writes",
                needed.next_power_of_two().max(MIN_SCRATCH),
                vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
                Placement::Device,
            )?;
            if let Some(previous) = self.scratch.replace(replacement) {
                ctx.retire.defer(ctx.frame_value, Box::new(previous));
            }
        }
        Ok(self
            .scratch
            .as_ref()
            .expect("the scratch buffer was just ensured"))
    }
}

fn packed_uv(uv: Vec2) -> u32 {
    u32::from(f16::from_f32(uv.x).to_bits()) | (u32::from(f16::from_f32(uv.y).to_bits()) << 16)
}
