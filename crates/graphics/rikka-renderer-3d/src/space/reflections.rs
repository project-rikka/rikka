use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::graph::{FrameGraph, ImageId, ImageSpec, Size, groups, scaled};
use crate::record::{Chain, SceneImages};

const FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;
const TILE: u32 = 8;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TracePush {
    frame: u64,
    source: u32,
    target: u32,
    size: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ResolvePush {
    frame: u64,
    source: u32,
    trace: u32,
    half_size: [u32; 2],
}

pub(super) fn trace(graph: &mut FrameGraph<'_>, scene: &SceneImages, chain: &Chain) -> ImageId {
    let half = scaled(graph.extent(), 2);
    let trace =
        graph.create(ImageSpec::new("rikka.space.trace", FORMAT, Size::Divided(2)).nearest());
    let source = chain.current();
    graph
        .compute("rikka.space.trace")
        .read(source)
        .read_all(scene.scene_reads())
        .read_all(scene.sky_reads())
        .write(trace)
        .record(move |record, ctx, scope| {
            record.dispatch_kernel(
                ctx,
                shaders_3d::SPACE_TRACE_COMP,
                &TracePush {
                    frame: scope.frame(),
                    source: scope.sampled(source),
                    target: scope.storage(trace),
                    size: [half.width, half.height],
                },
                groups(half, TILE),
            )
        });
    trace
}

pub(super) fn resolve(
    graph: &mut FrameGraph<'_>,
    scene: &SceneImages,
    chain: &mut Chain,
    trace: ImageId,
) {
    let half = scaled(graph.extent(), 2);
    let reads = scene.scene_reads().chain(scene.sky_reads()).chain([trace]);
    chain.filter(
        graph,
        "rikka.space.reflections",
        reads,
        move |record, ctx, scope, input| {
            let push = ResolvePush {
                frame: scope.frame(),
                source: input,
                trace: scope.sampled(trace),
                half_size: [half.width, half.height],
            };
            record.space_fullscreen(ctx, |space| &space.reflect, bytemuck::bytes_of(&push))
        },
    );
}
