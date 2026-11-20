use ash::vk;

use crate::error::Result;
use crate::fluid::plan::FluidFrame;
use crate::frame_data::FrameAddresses;
use crate::gpu::GpuPassPush;
use crate::graph::FrameGraph;
use crate::material::Cull;
use crate::pipeline::DrawState;
use crate::record::stage::{FluidImages, SceneImages};
use crate::record::{Chain, Record};
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

pub(crate) fn declare_composite(
    graph: &mut FrameGraph<'_>,
    scene: &SceneImages,
    images: FluidImages,
    chain: &mut Chain,
) {
    let reads = [
        images.depth,
        images.front,
        images.smoothed[0],
        images.thickness,
        images.scattering,
        images.coverage,
    ]
    .into_iter()
    .chain(scene.scene_reads())
    .chain(scene.sky_reads());
    chain.filter_drawing(
        graph,
        "rikka.fluid.composite",
        reads,
        scene.shadow_reads(),
        |record, ctx, scope, input| {
            record.fluid_composite(ctx, scope.fluid(), scope.addresses, input)
        },
    );
}

impl Record<'_> {
    fn fluid_composite(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        fluid: &FluidFrame,
        addresses: &FrameAddresses,
        input: u32,
    ) -> Result<()> {
        let composite = self
            .mesh
            .look_state
            .fluid
            .composite_recipe(&self.mesh.builtin, fluid.styled);
        self.fullscreen(
            ctx,
            &composite,
            DrawState::NO_DEPTH,
            bytemuck::bytes_of(&GpuPassPush {
                frame: addresses.refs,
                slot: input,
                flags: 0,
            }),
        )?;
        if fluid.layers.iter().all(|layer| layer.spray_vertices == 0) {
            return Ok(());
        }
        let spray = self.mesh.look_state.fluid.spray_recipe();
        let pipeline = self.pipeline(&spray)?;
        self.bind(
            ctx,
            pipeline,
            DrawState::depth_tested(Cull::None, vk::CompareOp::ALWAYS, false),
        );
        for (index, layer) in fluid.layers.iter().enumerate() {
            if layer.spray_vertices == 0 {
                continue;
            }
            self.push(
                ctx,
                &GpuPassPush {
                    frame: addresses.refs,
                    slot: index as u32,
                    flags: 0,
                },
            );
            unsafe {
                ctx.device
                    .handle()
                    .cmd_draw(ctx.cmd, layer.spray_vertices, 1, 0, 0);
            }
        }
        Ok(())
    }
}
