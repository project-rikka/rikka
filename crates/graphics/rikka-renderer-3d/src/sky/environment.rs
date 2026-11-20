use ash::vk;

use crate::graph::{Buffer, BufferUse, FrameGraph};
use crate::record::stage::SkyImages;
use crate::sky::images::{ENVIRONMENT_LEVELS, ENVIRONMENT_SIZE};
use crate::sky::kernel::{KernelPush, PrefilterPush, covering};

pub(super) fn declare_environment(graph: &mut FrameGraph<'_>, sky: SkyImages) {
    let luts = [sky.transmittance, sky.multiscatter, sky.view];
    graph
        .compute("rikka.three.sky.harmonics")
        .read_all(luts)
        .buffer(
            Buffer::Environment,
            BufferUse::storage_write(vk::PipelineStageFlags2::COMPUTE_SHADER),
        )
        .record(move |record, ctx, scope| {
            record.dispatch_kernel(
                ctx,
                shaders_3d::SKY_HARMONICS_COMP,
                &KernelPush {
                    frame: scope.frame(),
                    target: 0,
                    pad: 0,
                },
                (1, 1),
            )
        });
    let base = sky.environment_base;
    graph
        .compute("rikka.three.sky.environment")
        .read_all(luts)
        .write(base)
        .record(move |record, ctx, scope| {
            record.dispatch_kernel(
                ctx,
                shaders_3d::SKY_ENVIRONMENT_COMP,
                &KernelPush {
                    frame: scope.frame(),
                    target: scope.storage(base),
                    pad: 0,
                },
                covering((ENVIRONMENT_SIZE, ENVIRONMENT_SIZE)),
            )
        });
    let environment = sky.environment;
    graph
        .compute("rikka.three.sky.prefilter")
        .read(base)
        .write(environment)
        .record(move |record, ctx, scope| {
            record.dispatch_kernel(
                ctx,
                shaders_3d::SKY_PREFILTER_COMP,
                &PrefilterPush {
                    frame: scope.frame(),
                    target: scope.storage(environment),
                    source: scope.sampled(base),
                    levels: ENVIRONMENT_LEVELS,
                    pad: 0,
                },
                covering((ENVIRONMENT_SIZE + ENVIRONMENT_SIZE / 2, ENVIRONMENT_SIZE)),
            )
        });
}
