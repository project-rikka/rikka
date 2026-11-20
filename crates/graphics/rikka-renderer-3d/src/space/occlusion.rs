use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::graph::{FrameGraph, ImageSpec, Size, groups, scaled};
use crate::record::SceneImages;

const FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;
const TILE: u32 = 8;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct HorizonPush {
    frame: u64,
    target: u32,
    size: [u32; 2],
    _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DenoisePush {
    frame: u64,
    source: u32,
    target: u32,
    size: [u32; 2],
    half_size: [u32; 2],
}

pub(super) fn declare(graph: &mut FrameGraph<'_>, scene: &mut SceneImages) {
    let extent = graph.extent();
    let half = scaled(extent, 2);
    let horizons =
        graph.create(ImageSpec::new("rikka.space.horizons", FORMAT, Size::Divided(2)).nearest());
    let occlusion = graph.create(ImageSpec::new(
        "rikka.space.occlusion",
        FORMAT,
        Size::Viewport,
    ));
    graph.publish(occlusion);
    let reads = scene.scene_reads();
    graph.grouped("rikka.three.occlusion", |graph| {
        graph
            .compute("rikka.space.horizons")
            .read_all(reads)
            .write(horizons)
            .record(move |record, ctx, scope| {
                record.dispatch_kernel(
                    ctx,
                    shaders_3d::SPACE_OCCLUSION_COMP,
                    &HorizonPush {
                        frame: scope.frame(),
                        target: scope.storage(horizons),
                        size: [half.width, half.height],
                        _pad: 0,
                    },
                    groups(half, TILE),
                )
            });
        graph
            .compute("rikka.space.denoise")
            .read_all(scene.scene_reads())
            .read(horizons)
            .write(occlusion)
            .record(move |record, ctx, scope| {
                record.dispatch_kernel(
                    ctx,
                    shaders_3d::SPACE_DENOISE_COMP,
                    &DenoisePush {
                        frame: scope.frame(),
                        source: scope.sampled(horizons),
                        target: scope.storage(occlusion),
                        size: [extent.width, extent.height],
                        half_size: [half.width, half.height],
                    },
                    groups(extent, TILE),
                )
            });
    });
    scene.occlusion = Some(occlusion);
}
