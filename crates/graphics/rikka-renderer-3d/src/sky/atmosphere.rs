use crate::graph::{FrameGraph, ImageId};
use crate::record::stage::SkyImages;
use crate::sky::images::{AERIAL_CELLS, MULTISCATTER_SIZE, SKY_VIEW_SIZE, TRANSMITTANCE_SIZE};
use crate::sky::kernel::{KernelPush, covering};
use rikka_spirv::Shader;

fn declare_table(
    graph: &mut FrameGraph<'_>,
    name: &'static str,
    target: ImageId,
    reads: &[ImageId],
    kernel: Shader,
    groups: (u32, u32),
) {
    graph
        .compute(name)
        .read_all(reads.iter().copied())
        .write(target)
        .record(move |record, ctx, scope| {
            record.dispatch_kernel(
                ctx,
                kernel,
                &KernelPush {
                    frame: scope.frame(),
                    target: scope.storage(target),
                    pad: 0,
                },
                groups,
            )
        });
}

pub(super) fn declare_medium(graph: &mut FrameGraph<'_>, sky: SkyImages) {
    declare_table(
        graph,
        "rikka.three.sky.transmittance",
        sky.transmittance,
        &[],
        shaders_3d::SKY_TRANSMITTANCE_COMP,
        covering(TRANSMITTANCE_SIZE),
    );
    declare_table(
        graph,
        "rikka.three.sky.multiscatter",
        sky.multiscatter,
        &[sky.transmittance],
        shaders_3d::SKY_MULTISCATTER_COMP,
        MULTISCATTER_SIZE,
    );
}

pub(super) fn declare_view(graph: &mut FrameGraph<'_>, sky: SkyImages) {
    declare_table(
        graph,
        "rikka.three.sky.view",
        sky.view,
        &[sky.transmittance, sky.multiscatter],
        shaders_3d::SKY_VIEW_COMP,
        covering(SKY_VIEW_SIZE),
    );
}

pub(super) fn declare_aerial(graph: &mut FrameGraph<'_>, sky: SkyImages, aerial: ImageId) {
    declare_table(
        graph,
        "rikka.three.sky.aerial",
        aerial,
        &[sky.transmittance, sky.multiscatter],
        shaders_3d::SKY_AERIAL_COMP,
        covering((AERIAL_CELLS, AERIAL_CELLS)),
    );
}
