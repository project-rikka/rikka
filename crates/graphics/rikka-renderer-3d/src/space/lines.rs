use bytemuck::{Pod, Zeroable};

use crate::graph::FrameGraph;
use crate::record::{Chain, SceneImages};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LinesPush {
    frame: u64,
    source: u32,
    _pad: u32,
}

pub(super) fn declare(graph: &mut FrameGraph<'_>, scene: &SceneImages, chain: &mut Chain) {
    chain.filter(
        graph,
        "rikka.space.lines",
        scene.scene_reads(),
        move |record, ctx, scope, source| {
            let push = LinesPush {
                frame: scope.frame(),
                source,
                _pad: 0,
            };
            record.space_fullscreen(ctx, |space| &space.lines, bytemuck::bytes_of(&push))
        },
    );
}
