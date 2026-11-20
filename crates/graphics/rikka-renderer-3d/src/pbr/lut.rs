use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::graph::{FrameGraph, ImageSpec, Size, groups};
use crate::record::{Record, SceneImages};

const SIZE: u32 = 32;
const SAMPLES: u32 = 1024;
const TILE: u32 = 8;
const FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LutPush {
    target: u32,
    size: u32,
    samples: u32,
    _pad: u32,
}

const NAME: &str = "rikka.three.pbr.brdf";

impl Record<'_> {
    pub(crate) fn declare_pbr_lut(&mut self, graph: &mut FrameGraph<'_>, scene: &mut SceneImages) {
        let lut = graph.create(ImageSpec::new(NAME, FORMAT, Size::Fixed(SIZE, SIZE)).shared());
        graph.publish(lut);
        scene.brdf = Some(lut);
        if self.mesh.look_state.images.holds_shared(NAME) {
            return;
        }
        graph
            .compute(NAME)
            .write(lut)
            .record(move |record, ctx, scope| {
                record.dispatch_kernel(
                    ctx,
                    shaders_3d::PBR_BRDF_COMP,
                    &LutPush {
                        target: scope.storage(lut),
                        size: SIZE,
                        samples: SAMPLES,
                        _pad: 0,
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
    }
}
