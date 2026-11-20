use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::composite::DepthOfField;
use crate::graph::{FrameGraph, ImageId, ImageSpec, Size, scaled};
use crate::post::kernel::{covering, size_of_extent};
use crate::post::optics::Focus;
use crate::record::{Chain, SceneImages, Stage, Written};
use crate::viewport::COLOR_FORMAT;

const TILE: u32 = 8;
const HIGHLIGHTS: f32 = 3.0;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SetupPush {
    source: u32,
    depth: u32,
    color: u32,
    coc: u32,
    size: [u32; 2],
    full: [u32; 2],
    scale: f32,
    ratio: f32,
    limit: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TilesPush {
    coc: u32,
    tiles: u32,
    size: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DilatePush {
    tiles: u32,
    dilated: u32,
    size: [u32; 2],
    reach: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GatherPush {
    color: u32,
    coc: u32,
    tiles: u32,
    far: u32,
    near: u32,
    coverage: u32,
    seed: u32,
    size: [u32; 2],
    blades: f32,
    rotation: f32,
    highlights: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CombinePush {
    source: u32,
    destination: u32,
    depth: u32,
    far: u32,
    near: u32,
    coverage: u32,
    coc: u32,
    size: [u32; 2],
    half_size: [u32; 2],
    scale: f32,
    ratio: f32,
    limit: f32,
}

#[derive(Clone, Copy)]
struct Layers {
    far: ImageId,
    near: ImageId,
    coverage: ImageId,
    coc: ImageId,
}

fn half_image(graph: &mut FrameGraph<'_>, name: &'static str, format: vk::Format) -> ImageId {
    graph.create(ImageSpec::new(name, format, Size::Divided(2)))
}

pub(crate) fn declare<'f>(
    graph: &mut FrameGraph<'f>,
    stage: Stage<'f>,
    scene: &SceneImages,
    input: ImageId,
    lens: &'f DepthOfField,
) -> ImageId {
    let extent = graph.extent();
    let Some(focus) = Focus::of(lens, &stage.frame.camera, extent.height) else {
        return input;
    };
    let depth = scene
        .display_depth()
        .expect("looked frames have a scene depth");
    let mut chain = Chain::start(input);
    let layers = declare_layers(graph, stage, lens, input, focus, depth);
    let half = scaled(extent, 2);
    let reads = [depth, layers.far, layers.near, layers.coverage, layers.coc];
    chain.filter_compute(
        graph,
        "rikka.three.post.focus_combine",
        reads,
        move |record, ctx, scope, written: Written| {
            let push = CombinePush {
                source: written.input,
                destination: written.storage,
                depth: scope.sampled(depth),
                far: scope.sampled(layers.far),
                near: scope.sampled(layers.near),
                coverage: scope.sampled(layers.coverage),
                coc: scope.sampled(layers.coc),
                size: size_of_extent(written.extent),
                half_size: size_of_extent(half),
                scale: focus.scale,
                ratio: focus.ratio,
                limit: focus.limit,
            };
            record.dispatch_kernel(
                ctx,
                shaders_3d::POST_FOCUS_COMBINE_COMP,
                &push,
                covering(written.extent),
            )
        },
    );
    chain.current()
}

fn declare_layers<'f>(
    graph: &mut FrameGraph<'f>,
    stage: Stage<'f>,
    lens: &'f DepthOfField,
    source: ImageId,
    focus: Focus,
    depth: ImageId,
) -> Layers {
    let extent = graph.extent();
    let half = scaled(extent, 2);
    let tiled = scaled(half, TILE);
    let color = half_image(graph, "rikka.three.post.focus_color", COLOR_FORMAT);
    let coc = half_image(graph, "rikka.three.post.focus_coc", vk::Format::R32_SFLOAT);
    graph
        .compute("rikka.three.post.focus_setup")
        .read(source)
        .read(depth)
        .write(color)
        .write(coc)
        .record(move |record, ctx, scope| {
            let setup = SetupPush {
                source: scope.sampled(source),
                depth: scope.sampled(depth),
                color: scope.storage(color),
                coc: scope.storage(coc),
                size: size_of_extent(half),
                full: size_of_extent(extent),
                scale: focus.scale,
                ratio: focus.ratio,
                limit: focus.limit,
            };
            record.dispatch_kernel(
                ctx,
                shaders_3d::POST_FOCUS_SETUP_COMP,
                &setup,
                covering(half),
            )
        });

    let tiled_size = Size::Fixed(tiled.width, tiled.height);
    let tiles = graph.create(ImageSpec::new(
        "rikka.three.post.focus_tiles",
        COLOR_FORMAT,
        tiled_size,
    ));
    graph
        .compute("rikka.three.post.focus_tiles")
        .read(coc)
        .write(tiles)
        .record(move |record, ctx, scope| {
            let push = TilesPush {
                coc: scope.sampled(coc),
                tiles: scope.storage(tiles),
                size: size_of_extent(half),
            };
            record.dispatch_kernel(
                ctx,
                shaders_3d::POST_FOCUS_TILES_COMP,
                &push,
                (tiled.width, tiled.height),
            )
        });

    let dilated = graph.create(ImageSpec::new(
        "rikka.three.post.focus_dilated",
        COLOR_FORMAT,
        tiled_size,
    ));
    graph
        .compute("rikka.three.post.focus_dilate")
        .read(tiles)
        .write(dilated)
        .record(move |record, ctx, scope| {
            let push = DilatePush {
                tiles: scope.sampled(tiles),
                dilated: scope.storage(dilated),
                size: size_of_extent(tiled),
                reach: (0.5 * focus.limit / TILE as f32).ceil() as i32,
            };
            record.dispatch_kernel(
                ctx,
                shaders_3d::POST_FOCUS_DILATE_COMP,
                &push,
                covering(tiled),
            )
        });

    let far = half_image(graph, "rikka.three.post.focus_far", COLOR_FORMAT);
    let near = half_image(graph, "rikka.three.post.focus_near", COLOR_FORMAT);
    let coverage = half_image(
        graph,
        "rikka.three.post.focus_coverage",
        vk::Format::R32_SFLOAT,
    );
    let seed = stage.frame.time.to_bits();
    graph
        .compute("rikka.three.post.focus_gather")
        .read(color)
        .read(coc)
        .read(dilated)
        .write(far)
        .write(near)
        .write(coverage)
        .record(move |record, ctx, scope| {
            let gather = GatherPush {
                color: scope.sampled(color),
                coc: scope.sampled(coc),
                tiles: scope.sampled(dilated),
                far: scope.storage(far),
                near: scope.storage(near),
                coverage: scope.storage(coverage),
                seed,
                size: size_of_extent(half),
                blades: lens.blades as f32,
                rotation: lens.blade_rotation,
                highlights: HIGHLIGHTS,
            };
            record.dispatch_kernel(
                ctx,
                shaders_3d::POST_FOCUS_GATHER_COMP,
                &gather,
                covering(half),
            )
        });
    Layers {
        far,
        near,
        coverage,
        coc,
    }
}
