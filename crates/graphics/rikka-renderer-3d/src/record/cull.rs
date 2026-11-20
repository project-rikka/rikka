use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::build::BuiltFrame;
use crate::error::Result;
use crate::frame_data::{COUNTER_LATE_DISPATCH, COUNTER_PYRAMID, FrameAddresses};
use crate::gpu::{GpuPassPush, element_address};
use crate::graph::{Buffer, BufferUse, FrameGraph, SHADER_STAGES};
use crate::pipeline::Compute;
use crate::record::command::{bind_compute, dispatch, push_constants};
use crate::record::deform::declare_deform;
use crate::record::depth::Phase;
use crate::record::stage::{HzbFacts, PYRAMID_LEVELS, SceneImages, Stage};
use crate::record::{PASS_LATE, Record};
use rikka_gpu::core::compute::{groups, widest_row};
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

const CULL_GROUP: u32 = 64;

const PYRAMID_TILE: u32 = 32;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PyramidPush {
    counter: u64,
    source: u32,
    source_size: [u32; 2],
    size: [u32; 2],
    levels: u32,
    groups: u32,
    destinations: [u32; PYRAMID_LEVELS],
    _pad: u32,
}

pub(crate) fn draws_read() -> [(Buffer, BufferUse); 4] {
    [
        (Buffer::Draws, BufferUse::indirect()),
        (Buffer::Draws, BufferUse::storage_read(SHADER_STAGES)),
        (Buffer::Deform, BufferUse::storage_read(SHADER_STAGES)),
        (
            Buffer::Clusters,
            BufferUse::storage_read(vk::PipelineStageFlags2::FRAGMENT_SHADER),
        ),
    ]
}

fn compute_write() -> BufferUse {
    BufferUse::storage_write(vk::PipelineStageFlags2::COMPUTE_SHADER)
}

impl Record<'_> {
    pub(super) fn declare_geometry<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        scene: &SceneImages,
    ) {
        let built = stage.built;
        declare_deform(graph, built);
        if !built.instances.is_empty() {
            let mut cull = graph
                .compute("rikka.three.cull")
                .buffer(
                    Buffer::Deform,
                    BufferUse::storage_read(vk::PipelineStageFlags2::COMPUTE_SHADER),
                )
                .buffer(Buffer::Draws, compute_write());
            if let Some(hzb) = scene.hzb {
                cull = cull.read(hzb);
            }
            cull.record(move |record, ctx, scope| record.cull_early(ctx, built, scope.addresses));
        }
        let cluster_counts = stage.view.cluster_counts;
        if stage.clustered {
            graph
                .compute("rikka.three.cluster")
                .buffer(Buffer::Clusters, compute_write())
                .record(move |record, ctx, scope| {
                    record.cluster(ctx, cluster_counts, scope.addresses)
                });
        }
        let late = built.layout.late_base.is_some();
        self.declare_prepass(graph, stage, scene, Phase::Early { late });
        let (Some(hzb_image), Some(hzb)) = (scene.hzb, stage.view.hzb) else {
            return;
        };
        if !late {
            return;
        }
        let depth_slot_image = scene.depth_resolved;
        graph
            .compute("rikka.three.pyramid")
            .read(depth_slot_image)
            .write(hzb_image)
            .buffer(
                Buffer::Draws,
                BufferUse::new(
                    vk::PipelineStageFlags2::COMPUTE_SHADER,
                    vk::AccessFlags2::SHADER_STORAGE_READ | vk::AccessFlags2::SHADER_STORAGE_WRITE,
                ),
            )
            .record(move |record, ctx, scope| {
                record.build_pyramid(ctx, hzb, scope.sampled(depth_slot_image), scope.addresses)
            });
        if !built.instances.is_empty() {
            graph
                .compute("rikka.three.late_cull")
                .read(hzb_image)
                .buffer(Buffer::Draws, BufferUse::indirect())
                .buffer(Buffer::Draws, compute_write())
                .record(move |record, ctx, scope| record.cull_late(ctx, built, scope.addresses));
        }
        self.declare_prepass(graph, stage, scene, Phase::Late);
    }

    fn cull_early(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        built: &BuiltFrame,
        addresses: &FrameAddresses,
    ) -> Result<()> {
        let layout = built.layout;
        bind_compute(ctx, self.mesh.pipelines.compute(Compute::Cull)?);
        let push = GpuPassPush {
            frame: addresses.refs,
            slot: layout.shadow_base,
            flags: 0,
        };
        push_constants(ctx, self.mesh.pipelines.layout(), bytemuck::bytes_of(&push));
        let spread = groups(built.instances.len() as u32, CULL_GROUP).min(widest_row(ctx.device));
        dispatch(ctx, spread, 1, 1 + layout.shadow_views);
        Ok(())
    }

    fn cull_late(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        built: &BuiltFrame,
        addresses: &FrameAddresses,
    ) -> Result<()> {
        let Some(late_base) = built.layout.late_base else {
            return Ok(());
        };
        bind_compute(ctx, self.mesh.pipelines.compute(Compute::Cull)?);
        let push = GpuPassPush {
            frame: addresses.refs,
            slot: late_base,
            flags: PASS_LATE,
        };
        push_constants(ctx, self.mesh.pipelines.layout(), bytemuck::bytes_of(&push));
        unsafe {
            ctx.device.handle().cmd_dispatch_indirect(
                ctx.cmd,
                addresses.counters_buffer,
                element_address::<u32>(0, COUNTER_LATE_DISPATCH),
            );
        }
        Ok(())
    }

    fn build_pyramid(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        hzb: HzbFacts,
        depth_slot: u32,
        addresses: &FrameAddresses,
    ) -> Result<()> {
        let tiles = [
            groups(hzb.extent.width, PYRAMID_TILE),
            groups(hzb.extent.height, PYRAMID_TILE),
        ];
        let push = PyramidPush {
            counter: element_address::<u32>(addresses.counters, COUNTER_PYRAMID),
            source: depth_slot,
            source_size: [hzb.source.width, hzb.source.height],
            size: [hzb.extent.width, hzb.extent.height],
            levels: hzb.levels.min(PYRAMID_LEVELS as u32),
            groups: tiles[0] * tiles[1],
            destinations: hzb.level_slots,
            _pad: 0,
        };
        let pipeline = self.mesh.pipelines.compute(Compute::Pyramid)?;
        bind_compute(ctx, pipeline);
        push_constants(ctx, self.mesh.pipelines.layout(), bytemuck::bytes_of(&push));
        dispatch(ctx, tiles[0], tiles[1], 1);
        Ok(())
    }

    fn cluster(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        counts: [u32; 3],
        addresses: &FrameAddresses,
    ) -> Result<()> {
        bind_compute(ctx, self.mesh.pipelines.compute(Compute::Cluster)?);
        let push = GpuPassPush {
            frame: addresses.refs,
            slot: 0,
            flags: 0,
        };
        push_constants(ctx, self.mesh.pipelines.layout(), bytemuck::bytes_of(&push));
        dispatch(
            ctx,
            groups(counts[0], 4),
            groups(counts[1], 4),
            groups(counts[2], 4),
        );
        Ok(())
    }
}
