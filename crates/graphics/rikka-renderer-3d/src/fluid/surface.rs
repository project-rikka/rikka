use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::error::Result;
use crate::fluid::plan::FluidFrame;
use crate::fluid::targets::THICKNESS_DIVISOR;
use crate::frame_data::FrameAddresses;
use crate::gpu::GpuPassPush;
use crate::graph::{Attachment, FrameGraph, ImageId, groups, scaled};
use crate::material::Cull;
use crate::pipeline::DrawState;
use crate::record::Record;
use crate::record::stage::{FluidImages, SceneImages};
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

const TILE: u32 = 8;

const MODE_FIRST: u32 = 0;
const MODE_ACROSS: u32 = 1;
const MODE_DOWN: u32 = 2;
const MODE_CLEAN: u32 = 3;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurPush {
    frame: u64,
    sources: [u32; 2],
    coverage: u32,
    seeding: u32,
    targets: [u32; 3],
    pad: u32,
    direction: [i32; 2],
    size: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SmoothPush {
    frame: u64,
    source: u32,
    target: u32,
    mode: u32,
    pad: u32,
    size: [u32; 2],
}

fn clear() -> vk::ClearColorValue {
    vk::ClearColorValue { float32: [0.0; 4] }
}

pub(crate) fn declare_surface(
    graph: &mut FrameGraph<'_>,
    scene: &SceneImages,
    images: FluidImages,
) {
    graph
        .render("rikka.fluid.depth")
        .color(Attachment::clear_color(images.front, clear()))
        .depth(Attachment::new(
            images.depth,
            crate::graph::Load::Clear(vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 0.0,
                    stencil: 0,
                },
            }),
        ))
        .read_all(scene.scene_depth)
        .record(|record, ctx, scope| record.fluid_depth(ctx, scope.fluid(), scope.addresses));
    let [splatted_thickness, splatted_scattering] = images.splatted;
    graph
        .render("rikka.fluid.thickness")
        .color(Attachment::clear_color(splatted_thickness, clear()))
        .color(Attachment::clear_color(splatted_scattering, clear()))
        .read(images.depth)
        .record(|record, ctx, scope| record.fluid_thickness(ctx, scope.fluid(), scope.addresses));
    declare_blur(graph, images);
    declare_smooth(graph, images);
}

fn declare_blur(graph: &mut FrameGraph<'_>, images: FluidImages) {
    let half = scaled(graph.extent(), THICKNESS_DIVISOR);
    let passes = [
        (
            "rikka.fluid.blur_across",
            images.splatted,
            None,
            images.spread,
            [1, 0],
        ),
        (
            "rikka.fluid.blur_down",
            [images.spread[0], images.spread[1]],
            Some(images.spread[2]),
            [images.thickness, images.scattering, images.coverage],
            [0, 1],
        ),
    ];
    graph.grouped("rikka.fluid.blur", |graph| {
        for (name, sources, coverage, targets, direction) in passes {
            let mut pass = graph
                .compute(name)
                .read_all(sources)
                .read_all(coverage)
                .read(images.depth);
            for target in targets {
                pass = pass.write(target);
            }
            pass.record(move |record, ctx, scope| {
                record.dispatch_kernel(
                    ctx,
                    shaders_3d::FLUID_BLUR_COMP,
                    &BlurPush {
                        frame: scope.frame(),
                        sources: sources.map(|source| scope.sampled(source)),
                        coverage: coverage.map_or(0, |coverage| scope.sampled(coverage)),
                        seeding: u32::from(coverage.is_none()),
                        targets: targets.map(|target| scope.storage(target)),
                        pad: 0,
                        direction,
                        size: [half.width, half.height],
                    },
                    groups(half, TILE),
                )
            });
        }
    });
}

fn declare_smooth(graph: &mut FrameGraph<'_>, images: FluidImages) {
    let extent = graph.extent();
    let [first, second] = images.smoothed;
    let steps: [(&'static str, u32, ImageId, ImageId); 5] = [
        ("rikka.fluid.smooth_first", MODE_FIRST, images.depth, first),
        ("rikka.fluid.smooth_down", MODE_DOWN, first, second),
        ("rikka.fluid.smooth_across", MODE_ACROSS, second, first),
        ("rikka.fluid.smooth_down", MODE_DOWN, first, second),
        ("rikka.fluid.smooth_clean", MODE_CLEAN, second, first),
    ];
    graph.grouped("rikka.fluid.smooth", |graph| {
        for (name, mode, source, target) in steps {
            graph
                .compute(name)
                .read(source)
                .read(images.depth)
                .read(images.coverage)
                .write(target)
                .record(move |record, ctx, scope| {
                    record.dispatch_kernel(
                        ctx,
                        shaders_3d::FLUID_SMOOTH_COMP,
                        &SmoothPush {
                            frame: scope.frame(),
                            source: scope.sampled(source),
                            target: scope.storage(target),
                            mode,
                            pad: 0,
                            size: [extent.width, extent.height],
                        },
                        groups(extent, TILE),
                    )
                });
        }
    });
}

impl Record<'_> {
    fn fluid_depth(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        fluid: &FluidFrame,
        addresses: &FrameAddresses,
    ) -> Result<()> {
        let seed = self.mesh.look_state.fluid.seed_recipe(&self.mesh.builtin);
        self.fullscreen(
            ctx,
            &seed,
            DrawState::depth_tested(Cull::None, vk::CompareOp::ALWAYS, true),
            bytemuck::bytes_of(&GpuPassPush {
                frame: addresses.refs,
                slot: 0,
                flags: 0,
            }),
        )?;
        let recipe = self.mesh.look_state.fluid.depth_recipe();
        let pipeline = self.pipeline(&recipe)?;
        self.bind(
            ctx,
            pipeline,
            DrawState::depth_tested(Cull::None, vk::CompareOp::GREATER, true),
        );
        self.fluid_splats(ctx, fluid, addresses, 0);
        Ok(())
    }

    fn fluid_thickness(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        fluid: &FluidFrame,
        addresses: &FrameAddresses,
    ) -> Result<()> {
        let recipe = self.mesh.look_state.fluid.thickness_recipe();
        let pipeline = self.pipeline(&recipe)?;
        self.bind(ctx, pipeline, DrawState::NO_DEPTH);
        self.fluid_splats(ctx, fluid, addresses, 0);
        Ok(())
    }

    pub(crate) fn fluid_splats(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        fluid: &FluidFrame,
        addresses: &FrameAddresses,
        view: u32,
    ) {
        for (index, layer) in fluid.layers.iter().enumerate() {
            if layer.vertices == 0 {
                continue;
            }
            self.push(
                ctx,
                &GpuPassPush {
                    frame: addresses.refs,
                    slot: index as u32,
                    flags: view,
                },
            );
            unsafe {
                ctx.device
                    .handle()
                    .cmd_draw(ctx.cmd, layer.vertices, 1, 0, 0);
            }
        }
    }
}
