use ash::vk;

use crate::gpu::GpuPassPush;
use crate::graph::{Attachment, FrameGraph, Load};
use crate::material::Cull;
use crate::pipeline::DrawState;
use crate::record::stage::{FluidImages, SceneImages};

pub(crate) fn declare_stamp(graph: &mut FrameGraph<'_>, scene: &SceneImages, images: FluidImages) {
    let (Some(depth), Some(surface)) = (scene.scene_depth, scene.surface) else {
        return;
    };
    graph
        .render("rikka.fluid.stamp")
        .color(Attachment::new(surface, Load::Keep))
        .depth(Attachment::new(depth, Load::Keep))
        .read_all([
            images.depth,
            images.front,
            images.smoothed[0],
            images.thickness,
            images.scattering,
            images.coverage,
        ])
        .record(|record, ctx, scope| {
            let recipe = record
                .mesh
                .look_state
                .fluid
                .stamp_recipe(&record.mesh.builtin);
            record.fullscreen(
                ctx,
                &recipe,
                DrawState::depth_tested(Cull::None, vk::CompareOp::GREATER, true),
                bytemuck::bytes_of(&GpuPassPush {
                    frame: scope.frame(),
                    slot: 0,
                    flags: 0,
                }),
            )
        });
}
