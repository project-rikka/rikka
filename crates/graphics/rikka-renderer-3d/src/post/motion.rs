use bytemuck::{Pod, Zeroable};

use crate::composite::MotionBlur;
use crate::gpu::NONE_INDEX;
use crate::graph::{FrameGraph, ImageId, ImageSpec, Size, scaled};
use crate::post::kernel::{covering, size_of_extent};
use crate::record::{Chain, SceneImages, Stage, Written};
use crate::viewport::COLOR_FORMAT;

const TILE: u32 = 32;
const LONGEST_SHUTTER: f32 = 2.0;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TilesPush {
    frame: u64,
    velocity: u32,
    tiles: u32,
    size: [u32; 2],
    shutter: f32,
    limit: f32,
    depth_source: u32,
    motion_source: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct NeighborsPush {
    tiles: u32,
    neighbors: u32,
    size: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurPush {
    source: u32,
    destination: u32,
    velocity: u32,
    neighbors: u32,
    size: [u32; 2],
    tile: u32,
    seed: u32,
}

pub(crate) fn declare<'f>(
    graph: &mut FrameGraph<'f>,
    stage: Stage<'f>,
    scene: &SceneImages,
    input: ImageId,
    blur: &MotionBlur,
) -> ImageId {
    let shutter = blur.amount;
    if !blur.is_active() {
        return input;
    }
    let mut chain = Chain::start(input);
    let extent = graph.extent();
    let post = scene.post;
    let tiled = scaled(extent, TILE);
    let velocity = graph.create(ImageSpec::new(
        "rikka.three.post.velocity",
        COLOR_FORMAT,
        Size::Viewport,
    ));
    let tiles = graph.create(ImageSpec::new(
        "rikka.three.post.motion_tiles",
        COLOR_FORMAT,
        Size::Divided(TILE),
    ));
    let neighbors = graph.create(ImageSpec::new(
        "rikka.three.post.motion_neighbors",
        COLOR_FORMAT,
        Size::Divided(TILE),
    ));
    graph
        .compute("rikka.three.post.motion_tiles")
        .read_all(scene.scene_reads())
        .write(velocity)
        .write(tiles)
        .record(move |record, ctx, scope| {
            let push = TilesPush {
                frame: scope.frame(),
                velocity: scope.storage(velocity),
                tiles: scope.storage(tiles),
                size: size_of_extent(extent),
                shutter: shutter.min(LONGEST_SHUTTER),
                limit: TILE as f32,
                depth_source: post.map_or(NONE_INDEX, |post| scope.sampled(post.depth)),
                motion_source: post.map_or(NONE_INDEX, |post| scope.sampled(post.motion)),
            };
            record.dispatch_kernel(
                ctx,
                shaders_3d::POST_MOTION_TILES_COMP,
                &push,
                (tiled.width, tiled.height),
            )
        });
    graph
        .compute("rikka.three.post.motion_neighbors")
        .read(tiles)
        .write(neighbors)
        .record(move |record, ctx, scope| {
            let push = NeighborsPush {
                tiles: scope.sampled(tiles),
                neighbors: scope.storage(neighbors),
                size: size_of_extent(tiled),
            };
            record.dispatch_kernel(
                ctx,
                shaders_3d::POST_MOTION_NEIGHBORS_COMP,
                &push,
                covering(tiled),
            )
        });
    let seed = stage.frame.time.to_bits();
    chain.filter_compute(
        graph,
        "rikka.three.post.motion_blur",
        [velocity, neighbors],
        move |record, ctx, scope, written: Written| {
            let push = BlurPush {
                source: written.input,
                destination: written.storage,
                velocity: scope.sampled(velocity),
                neighbors: scope.sampled(neighbors),
                size: size_of_extent(written.extent),
                tile: TILE,
                seed,
            };
            record.dispatch_kernel(
                ctx,
                shaders_3d::POST_MOTION_BLUR_COMP,
                &push,
                covering(written.extent),
            )
        },
    );
    chain.current()
}
