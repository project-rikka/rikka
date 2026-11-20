use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::graph::{Buffer, BufferUse, FrameGraph, ImageId};
use crate::record::stage::SkyImages;
use crate::sky::images::{ENVIRONMENT_LEVELS, ENVIRONMENT_SIZE, PANORAMA_SIZE};
use crate::sky::kernel::{PrefilterPush, covering};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PanoramaPush {
    frame: u64,
    target: u32,
    source: u32,
}

pub(super) fn declare_capture(graph: &mut FrameGraph<'_>, atlas: ImageId, source: u32) {
    graph
        .compute("rikka.three.sky.panorama.capture")
        .write(atlas)
        .record(move |record, ctx, scope| {
            record.dispatch_kernel(
                ctx,
                shaders_3d::SKY_PANORAMA_CAPTURE_COMP,
                &PanoramaPush {
                    frame: scope.frame(),
                    target: scope.storage(atlas),
                    source,
                },
                covering(PANORAMA_SIZE),
            )
        });
    graph
        .compute("rikka.three.sky.panorama.reduce")
        .modify(atlas)
        .record(move |record, ctx, scope| {
            record.dispatch_kernel(
                ctx,
                shaders_3d::SKY_PANORAMA_REDUCE_COMP,
                &PanoramaPush {
                    frame: scope.frame(),
                    target: scope.storage(atlas),
                    source: 0,
                },
                covering((PANORAMA_SIZE.0, PANORAMA_SIZE.1 / 2)),
            )
        });
}

pub(super) fn declare_lighting(graph: &mut FrameGraph<'_>, sky: SkyImages, atlas: ImageId) {
    graph
        .compute("rikka.three.sky.panorama.harmonics")
        .read(atlas)
        .buffer(
            Buffer::Environment,
            BufferUse::storage_write(vk::PipelineStageFlags2::COMPUTE_SHADER),
        )
        .record(move |record, ctx, scope| {
            record.dispatch_kernel(
                ctx,
                shaders_3d::SKY_PANORAMA_HARMONICS_COMP,
                &PanoramaPush {
                    frame: scope.frame(),
                    target: 0,
                    source: scope.sampled(atlas),
                },
                (1, 1),
            )
        });
    let environment = sky.environment;
    graph
        .compute("rikka.three.sky.panorama.prefilter")
        .read(atlas)
        .write(environment)
        .record(move |record, ctx, scope| {
            record.dispatch_kernel(
                ctx,
                shaders_3d::SKY_PANORAMA_PREFILTER_COMP,
                &PrefilterPush {
                    frame: scope.frame(),
                    target: scope.storage(environment),
                    source: scope.sampled(atlas),
                    levels: ENVIRONMENT_LEVELS,
                    pad: 0,
                },
                covering((ENVIRONMENT_SIZE + ENVIRONMENT_SIZE / 2, ENVIRONMENT_SIZE)),
            )
        });
}
