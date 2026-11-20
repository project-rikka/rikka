use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::graph::{FrameGraph, ImageSpec, Size, groups};
use crate::record::stage::LtcImages;
use crate::record::{Record, SceneImages};

const SIZE: u32 = 64;
const TILE: u32 = 8;
const FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LtcPush {
    frame: u64,
    inverse: u32,
    amplitude: u32,
}

impl Record<'_> {
    pub(super) fn declare_ltc(&mut self, graph: &mut FrameGraph<'_>, scene: &mut SceneImages) {
        let table = |graph: &mut FrameGraph<'_>, name| {
            let image =
                graph.create(ImageSpec::new(name, FORMAT, Size::Fixed(SIZE, SIZE)).shared());
            graph.publish(image);
            image
        };
        let ltc = LtcImages {
            inverse: table(graph, "rikka.three.ltc.inverse"),
            amplitude: table(graph, "rikka.three.ltc.amplitude"),
        };
        graph
            .compute("rikka.three.ltc")
            .modify(ltc.inverse)
            .modify(ltc.amplitude)
            .record(move |record, ctx, scope| {
                if !scope.fresh(ltc.inverse) && !scope.fresh(ltc.amplitude) {
                    return Ok(());
                }
                record.dispatch_kernel(
                    ctx,
                    shaders_3d::LTC_TABLE_COMP,
                    &LtcPush {
                        frame: scope.frame(),
                        inverse: scope.storage(ltc.inverse),
                        amplitude: scope.storage(ltc.amplitude),
                    },
                    groups(
                        vk::Extent2D {
                            width: SIZE,
                            height: SIZE,
                        },
                        TILE,
                    ),
                )
            });
        scene.ltc = Some(ltc);
    }
}
