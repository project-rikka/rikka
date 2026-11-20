use bytemuck::{Pod, Zeroable};

use crate::gpu::NONE_INDEX;
use crate::graph::{Buffer, BufferUse, FrameGraph, SHADER_STAGES};
use crate::look::Look;
use crate::pipeline::DrawState;
use crate::record::{Chain, SceneImages};
use crate::sky::images::{self, SHAFTS_SCALE, fog_grid, shafts_size};
use crate::sky::kernel::{KernelPush, covering};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ApplyPush {
    frame: u64,
    source: u32,
    shafts: u32,
    shafts_divisor: u32,
    pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShaftsPush {
    frame: u64,
    target: u32,
    pad: u32,
    size: [u32; 2],
}

pub(super) fn declare_froxels(graph: &mut FrameGraph<'_>, scene: &mut SceneImages) {
    let (fog, grid) = images::fog(graph);
    let groups = covering(fog_grid(graph.extent()));
    graph
        .compute("rikka.three.fog_volume")
        .read_all(scene.atlas)
        .read_all(
            scene
                .sky
                .into_iter()
                .flat_map(|sky| [sky.transmittance, sky.multiscatter, sky.view]),
        )
        .write(fog)
        .buffer(Buffer::Environment, BufferUse::storage_read(SHADER_STAGES))
        .record(move |record, ctx, scope| {
            record.dispatch_kernel(
                ctx,
                shaders_3d::FOG_VOLUME_COMP,
                &KernelPush {
                    frame: scope.frame(),
                    target: scope.storage(fog),
                    pad: 0,
                },
                groups,
            )
        });
    scene.fog = Some((fog, grid));
}

pub(super) fn declare_apply(
    graph: &mut FrameGraph<'_>,
    look: &Look,
    scene: &SceneImages,
    chain: &mut Chain,
) {
    let shafts = (look.environment.sky().is_some() && look.fog.shafts > 0.0).then(|| {
        let shafts = images::shafts(graph);
        let size = shafts_size(graph.extent());
        graph
            .compute("rikka.three.sky.shafts")
            .read_all(scene.scene_reads())
            .read_all(scene.sky_reads())
            .write(shafts)
            .record(move |record, ctx, scope| {
                record.dispatch_kernel(
                    ctx,
                    shaders_3d::SKY_SHAFTS_COMP,
                    &ShaftsPush {
                        frame: scope.frame(),
                        target: scope.storage(shafts),
                        pad: 0,
                        size: [size.0, size.1],
                    },
                    covering(size),
                )
            });
        shafts
    });
    let reads = scene.scene_reads().chain(scene.sky_reads()).chain(shafts);
    chain.filter(
        graph,
        "rikka.three.fog",
        reads,
        move |record, ctx, scope, source| {
            let recipe = record.mesh.look_state.sky.fog_recipe(&record.mesh.builtin);
            record.fullscreen(
                ctx,
                &recipe,
                DrawState::NO_DEPTH,
                bytemuck::bytes_of(&ApplyPush {
                    frame: scope.frame(),
                    source,
                    shafts: shafts.map_or(NONE_INDEX, |shafts| scope.sampled(shafts)),
                    shafts_divisor: SHAFTS_SCALE,
                    pad: 0,
                }),
            )
        },
    );
}
