use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::build::BuiltFrame;
use crate::error::Result;
use crate::frame_data::FrameAddresses;
use crate::gpu::{GpuPassPush, NONE_INDEX};
use crate::graph::{
    Attachment, Buffer, BufferUse, FrameGraph, ImageSpec, Load, SHADER_STAGES, Size, groups,
};
use crate::material::MaterialPass;
use crate::pipeline::DrawState;
use crate::recipes;
use crate::record::{Chain, PASS_MASKED, Record, SceneImages, Stage};
use crate::viewport::COLOR_FORMAT;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

const TILE: u32 = 8;
const REACH: f32 = 0.08;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurPush {
    frame: u64,
    source: u32,
    profile: u32,
    base: u32,
    target: u32,
    size: [u32; 2],
    direction: [f32; 2],
    reach: f32,
    _pad: u32,
}

pub(crate) fn scatters(stage: Stage<'_>, scene: &SceneImages) -> bool {
    stage.look.is_some()
        && scene.scene_depth.is_some()
        && stage.built.buckets.iter().any(|bucket| bucket.scatters)
}

pub(crate) fn refracts(stage: Stage<'_>) -> bool {
    stage.look.is_some() && stage.built.buckets.iter().any(|bucket| bucket.refracts)
}

impl Record<'_> {
    pub(crate) fn declare_pbr_scatter<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        scene: &mut SceneImages,
        chain: &mut Chain,
    ) {
        let Some(depth) = scene.scene_depth else {
            return;
        };
        let image = |graph: &mut FrameGraph<'f>, name| {
            graph.create(ImageSpec::new(name, COLOR_FORMAT, Size::Viewport).nearest())
        };
        let diffuse = image(graph, "rikka.three.scatter.diffuse");
        let profile = image(graph, "rikka.three.scatter.profile");
        let across = image(graph, "rikka.three.scatter.across");
        let clear = vk::ClearColorValue { float32: [0.0; 4] };
        let mut pass = graph
            .render("rikka.three.scatter")
            .color(Attachment::clear_color(diffuse, clear))
            .color(Attachment::clear_color(profile, clear))
            .depth(Attachment::new(depth, Load::Keep))
            .read_all(scene.sky_reads())
            .buffer(Buffer::Environment, BufferUse::storage_read(SHADER_STAGES));
        for (buffer, usage) in crate::record::draws_read() {
            pass = pass.buffer(buffer, usage);
        }
        let built = stage.built;
        pass.record(move |record, ctx, scope| record.pbr_scatter(ctx, built, scope.addresses));

        let extent = graph.extent();
        let reach = REACH * extent.height as f32;
        graph
            .compute("rikka.three.scatter.across")
            .read(diffuse)
            .read(profile)
            .read(depth)
            .write(across)
            .record(move |record, ctx, scope| {
                record.dispatch_kernel(
                    ctx,
                    shaders_3d::SCATTER_BLUR_COMP,
                    &BlurPush {
                        frame: scope.frame(),
                        source: scope.sampled(diffuse),
                        profile: scope.sampled(profile),
                        base: NONE_INDEX,
                        target: scope.storage(across),
                        size: [extent.width, extent.height],
                        direction: [1.0, 0.0],
                        reach,
                        _pad: 0,
                    },
                    groups(extent, TILE),
                )
            });
        chain.filter_compute(
            graph,
            "rikka.three.scatter.along",
            [across, profile, depth],
            move |record, ctx, scope, written| {
                record.dispatch_kernel(
                    ctx,
                    shaders_3d::SCATTER_BLUR_COMP,
                    &BlurPush {
                        frame: scope.frame(),
                        source: scope.sampled(across),
                        profile: scope.sampled(profile),
                        base: written.input,
                        target: written.storage,
                        size: [written.extent.width, written.extent.height],
                        direction: [0.0, 1.0],
                        reach,
                        _pad: 0,
                    },
                    groups(written.extent, TILE),
                )
            },
        );
        scene.scatter = Some(diffuse);
    }

    fn pbr_scatter(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        built: &BuiltFrame,
        addresses: &FrameAddresses,
    ) -> Result<()> {
        for (index, bucket) in built.buckets.iter().enumerate() {
            if !bucket.scatters {
                continue;
            }
            let recipe = recipes::scatter(
                &self.mesh.builtin,
                &bucket.program,
                bucket.state,
                bucket.features,
            );
            let pipeline = self.pipeline(&recipe)?;
            self.bind(
                ctx,
                pipeline,
                DrawState {
                    depth_write: false,
                    ..DrawState::from_material(bucket.state, vk::CompareOp::GREATER_OR_EQUAL)
                },
            );
            self.push(
                ctx,
                &GpuPassPush {
                    frame: addresses.refs,
                    slot: 0,
                    flags: match bucket.pass {
                        MaterialPass::Masked => PASS_MASKED,
                        _ => 0,
                    },
                },
            );
            for range in [Some(index as u32), built.layout.late(index as u32)]
                .into_iter()
                .flatten()
            {
                self.draw_range(
                    ctx,
                    addresses,
                    range,
                    built.bucket_table[range as usize],
                    bucket.indices,
                );
            }
        }
        Ok(())
    }
}
