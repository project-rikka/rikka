use ash::vk;
use bytemuck::{Pod, Zeroable};

use super::{EmitterFrame, MOTION_FORMAT};
use crate::error::Result;
use crate::frame_data::{EmitterAddresses, FrameAddresses};
use crate::graph::{Attachment, Buffer, BufferUse, FrameGraph, ImageSpec, Load, Size};
use crate::material::Cull;
use crate::pipeline::{DrawState, Recipe};
use crate::record::command::push_constants;
use crate::record::{Record, SceneImages, Stage};
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

const GROUP: u32 = 64;
const CORNERS: u32 = 6;
const EVALUATE: &str = "rikka.three.emitters.evaluate";
const MOTION: &str = "rikka.three.emitters.motion";
const LAYERS: &str = "rikka.three.emitters.layers";

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct EmitterPush {
    frame: u64,
    emitters: u64,
    keys: u64,
    particles: u64,
    emitter: u32,
    pad: u32,
}

impl EmitterPush {
    fn new(frame: u64, addresses: EmitterAddresses, emitter: u32) -> Self {
        Self {
            frame,
            emitters: addresses.emitters,
            keys: addresses.keys,
            particles: addresses.particles,
            emitter,
            pad: 0,
        }
    }
}

pub(crate) fn particles_read() -> BufferUse {
    BufferUse::storage_read(
        vk::PipelineStageFlags2::VERTEX_SHADER | vk::PipelineStageFlags2::FRAGMENT_SHADER,
    )
}

impl Record<'_> {
    pub(crate) fn declare_space_emitters<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        scene: &SceneImages,
    ) {
        let emitters = stage.emitters;
        if emitters.is_empty() {
            return;
        }
        let shaders = &self.mesh.look_state.space.emitters;
        let recipes = [
            Some(shaders.shade_recipe(stage.view.formats)),
            stage.temporal.map(|_| shaders.motion_recipe()),
            scene.layers.map(|_| shaders.layers_recipe()),
        ];
        for recipe in recipes.iter().flatten() {
            self.mesh.pipelines.prepare(recipe);
        }
        let compute = vk::PipelineStageFlags2::COMPUTE_SHADER;
        let mut pass = graph
            .compute(EVALUATE)
            .buffer(Buffer::Environment, BufferUse::storage_read(compute))
            .buffer(Buffer::Particles, BufferUse::storage_write(compute));
        if let Some(atlas) = scene.atlas {
            pass = pass.read(atlas);
        }
        pass.record(move |record, ctx, scope| {
            for drawn in emitters.drawn() {
                let push = EmitterPush::new(scope.frame(), scope.addresses.emitters, drawn.index);
                record.dispatch_kernel(
                    ctx,
                    shaders_3d::EMITTER_EVALUATE_COMP,
                    &push,
                    (drawn.capacity.div_ceil(GROUP), 1),
                )?;
            }
            Ok(())
        });
    }

    pub(crate) fn declare_space_emitter_targets<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        scene: &mut SceneImages,
    ) {
        let emitters = stage.emitters;
        let Some(depth) = scene.scene_depth.filter(|_| !emitters.is_empty()) else {
            return;
        };
        if stage.temporal.is_some() {
            let motion =
                graph.create(ImageSpec::new(MOTION, MOTION_FORMAT, Size::Viewport).nearest());
            graph
                .render(MOTION)
                .color(Attachment::clear_color(
                    motion,
                    vk::ClearColorValue { float32: [0.0; 4] },
                ))
                .read(depth)
                .buffer(Buffer::Particles, particles_read())
                .record(move |record, ctx, scope| {
                    let recipe = record.mesh.look_state.space.emitters.motion_recipe();
                    record.space_emitters_draw(
                        ctx,
                        &recipe,
                        DrawState::NO_DEPTH,
                        emitters,
                        scope.addresses,
                    )
                });
            scene.emitter_motion = Some(motion);
        }
        if let Some(layers) = scene.layers {
            graph
                .render(LAYERS)
                .color(Attachment::new(layers, Load::Keep))
                .read(depth)
                .buffer(Buffer::Particles, particles_read())
                .record(move |record, ctx, scope| {
                    let recipe = record.mesh.look_state.space.emitters.layers_recipe();
                    record.space_emitters_draw(
                        ctx,
                        &recipe,
                        DrawState::NO_DEPTH,
                        emitters,
                        scope.addresses,
                    )
                });
        }
    }

    pub(crate) fn space_emitters(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        stage: Stage<'_>,
        addresses: &FrameAddresses,
    ) -> Result<()> {
        let emitters = stage.emitters;
        if emitters.is_empty() {
            return Ok(());
        }
        let recipe = self
            .mesh
            .look_state
            .space
            .emitters
            .shade_recipe(stage.view.formats);
        self.space_emitters_draw(
            ctx,
            &recipe,
            DrawState::depth_tested(Cull::None, vk::CompareOp::GREATER_OR_EQUAL, false),
            emitters,
            addresses,
        )
    }

    fn space_emitters_draw(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        recipe: &Recipe,
        draw: DrawState,
        emitters: &EmitterFrame,
        addresses: &FrameAddresses,
    ) -> Result<()> {
        let pipeline = self.pipeline(recipe)?;
        self.bind(ctx, pipeline, draw);
        let layout = self.mesh.pipelines.layout();
        for drawn in emitters.drawn() {
            let push = EmitterPush::new(addresses.refs, addresses.emitters, drawn.index);
            push_constants(ctx, layout, bytemuck::bytes_of(&push));
            unsafe {
                ctx.device
                    .handle()
                    .cmd_draw(ctx.cmd, CORNERS, drawn.capacity, 0, 0);
            }
        }
        self.forget_bindings();
        Ok(())
    }
}
