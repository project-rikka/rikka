use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::build::{BuiltFrame, DrapeJob, SkinJob};
use crate::error::Result;
use crate::frame_data::FrameAddresses;
use crate::gpu::element_address;
use crate::graph::{Buffer, BufferUse, FrameGraph};
use crate::pipeline::Compute;
use crate::record::Record;
use crate::record::command::{bind_compute, dispatch, push_constants};
use crate::vertex::{GpuAttribute, GpuPosition};
use rikka_gpu::core::barrier::MemoryAccess;
use rikka_gpu::core::compute::{groups, rows, widest_row};
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SkinPush {
    frame: u64,
    positions_in: u64,
    attributes_in: u64,
    skins_in: u64,
    positions_out: u64,
    attributes_out: u64,
    morph_starts: u64,
    morph_deltas: u64,
    morph_weights: u64,
    vertex_count: u32,
    morph_count: u32,
    bone_offset: u32,
    flags: u32,
}

const SKIN_HAS_SKELETON: u32 = 1 << 0;

const SKIN_GROUP: u32 = 64;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DrapePush {
    positions: u64,
    attributes: u64,
    drape: u64,
    to_local: [[f32; 3]; 4],
    linear: [[f32; 3]; 3],
    vertex_count: u32,
}

const DRAPE_GROUP: u32 = 64;

pub(super) fn declare_deform<'f>(graph: &mut FrameGraph<'f>, built: &'f BuiltFrame) {
    if built.skin_jobs.is_empty() {
        return;
    }
    graph
        .compute("rikka.three.deform")
        .buffer(
            Buffer::Deform,
            BufferUse::storage_write(vk::PipelineStageFlags2::COMPUTE_SHADER),
        )
        .record(move |record, ctx, scope| record.deform(ctx, built, scope.addresses));
}

impl Record<'_> {
    fn deform(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        built: &BuiltFrame,
        addresses: &FrameAddresses,
    ) -> Result<()> {
        let pipeline = self.mesh.pipelines.compute(Compute::Skin)?;
        let layout = self.mesh.pipelines.layout();
        let widest = widest_row(ctx.device);
        bind_compute(ctx, pipeline);
        for job in &built.skin_jobs {
            let push = skin_push(job, addresses);
            push_constants(ctx, layout, bytemuck::bytes_of(&push));
            let (x, y) = rows(groups(job.vertex_count, SKIN_GROUP), widest);
            dispatch(ctx, x, y, 1);
        }
        if built.skin_jobs.iter().any(|job| job.drape.is_some()) {
            ctx.memory_barrier(
                MemoryAccess::new(
                    vk::PipelineStageFlags2::COMPUTE_SHADER,
                    vk::AccessFlags2::SHADER_STORAGE_WRITE,
                ),
                MemoryAccess::new(
                    vk::PipelineStageFlags2::COMPUTE_SHADER,
                    vk::AccessFlags2::SHADER_STORAGE_READ | vk::AccessFlags2::SHADER_STORAGE_WRITE,
                ),
            );
            bind_compute(ctx, self.mesh.pipelines.compute(Compute::Drape)?);
            for job in &built.skin_jobs {
                let Some(drape) = job.drape else {
                    continue;
                };
                let push = drape_push(job, drape, addresses);
                push_constants(ctx, layout, bytemuck::bytes_of(&push));
                let (x, y) = rows(groups(job.vertex_count, DRAPE_GROUP), widest);
                dispatch(ctx, x, y, 1);
            }
        }
        self.forget_bindings();
        Ok(())
    }
}

fn drape_push(job: &SkinJob, drape: DrapeJob, addresses: &FrameAddresses) -> DrapePush {
    let to_local = drape.to_local.to_cols_array_2d();
    DrapePush {
        positions: element_address::<GpuPosition>(addresses.deform_positions, job.out_offset),
        attributes: element_address::<GpuAttribute>(addresses.deform_attributes, job.out_offset),
        drape: drape.address,
        to_local: to_local.map(|[x, y, z, _]| [x, y, z]),
        linear: drape.linear.to_cols_array_2d(),
        vertex_count: job.vertex_count,
    }
}

fn skin_push(job: &SkinJob, addresses: &FrameAddresses) -> SkinPush {
    SkinPush {
        frame: addresses.refs,
        positions_in: job.positions_in,
        attributes_in: job.attributes_in,
        skins_in: job.skins_in,
        positions_out: element_address::<GpuPosition>(addresses.deform_positions, job.out_offset),
        attributes_out: element_address::<GpuAttribute>(
            addresses.deform_attributes,
            job.out_offset,
        ),
        morph_starts: job.morph_starts,
        morph_deltas: job.morph_deltas,
        morph_weights: element_address::<f32>(addresses.morph_weights, job.morph_weight_first),
        vertex_count: job.vertex_count,
        morph_count: job.morph_count,
        bone_offset: job.bone_offset,
        flags: match job.has_skeleton {
            true => SKIN_HAS_SKELETON,
            false => 0,
        },
    }
}
