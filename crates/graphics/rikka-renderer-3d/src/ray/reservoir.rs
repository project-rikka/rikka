use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::gpu::NONE_INDEX;
use crate::graph::{Buffer, FrameGraph, ImageId, ImageSpec, Size, groups};
use crate::look::Illumination;
use crate::record::SceneImages;

use super::denoise::TILE;
use super::{samples, traced};

const KEY: u64 = 0x7261_7920;
const SAMPLE_FORMAT: vk::Format = vk::Format::R32G32B32A32_SFLOAT;
const STATE_FORMAT: vk::Format = vk::Format::R32G32B32A32_UINT;
const SAMPLES: &str = "rikka.ray.illumination.reservoir.samples";
const STATES: &str = "rikka.ray.illumination.reservoir.states";

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ReservoirPush {
    frame: u64,
    samples: u32,
    reach: f32,
    size: [u32; 2],
    radiance: u32,
    guide_history: u32,
    sample_target: u32,
    state_target: u32,
    sample_history: u32,
    state_history: u32,
    temporal: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ResamplePush {
    frame: u64,
    target: u32,
    guide: u32,
    samples: u32,
    states: u32,
    size: [u32; 2],
    reach: f32,
    spatial: u32,
}

#[derive(Clone, Copy, Debug)]
struct Reservoirs {
    samples: ImageId,
    states: ImageId,
}

impl Reservoirs {
    fn declare(graph: &mut FrameGraph<'_>, previous: bool) -> Self {
        let spec = |name, format| {
            let spec = ImageSpec::new(name, format, Size::Divided(2)).nearest();
            match previous {
                true => spec.previous(KEY),
                false => spec.current(KEY),
            }
        };
        Self {
            samples: graph.create(spec(SAMPLES, SAMPLE_FORMAT)),
            states: graph.create(spec(STATES, STATE_FORMAT)),
        }
    }
}

pub(super) fn resample(
    graph: &mut FrameGraph<'_>,
    scene: &SceneImages,
    illumination: Illumination,
    radiance: ImageId,
    target: ImageId,
) {
    let guides = scene.guides.expect("illumination declares guides");
    let extent = Size::Divided(2).resolve(graph.extent());
    let current = Reservoirs::declare(graph, false);
    let previous = illumination
        .resampling
        .temporal()
        .then(|| Reservoirs::declare(graph, true));
    let held = previous.map(|previous| [previous.samples, previous.states]);
    let pass = graph
        .compute("rikka.ray.illumination.sample")
        .read_all(scene.scene_reads())
        .read_all(scene.sky_reads())
        .read(radiance)
        .read(guides.half_history)
        .read_all(held.into_iter().flatten())
        .buffer(Buffer::Scene, traced())
        .write(current.samples)
        .write(current.states);
    let guide_history = guides.half_history;
    pass.record(move |record, ctx, scope| {
        let stale = scope.fresh(radiance) || scope.fresh(guide_history);
        let history = previous.filter(|previous| {
            !stale && !scope.fresh(previous.samples) && !scope.fresh(previous.states)
        });
        record.dispatch_kernel(
            ctx,
            shaders_3d::RAY_RESERVOIR_COMP,
            &ReservoirPush {
                frame: scope.frame(),
                samples: samples(illumination.samples),
                reach: illumination.reach.max(0.01),
                size: [extent.width, extent.height],
                radiance: match stale {
                    true => NONE_INDEX,
                    false => scope.sampled(radiance),
                },
                guide_history: scope.sampled(guide_history),
                sample_target: scope.storage(current.samples),
                state_target: scope.storage(current.states),
                sample_history: history
                    .map_or(NONE_INDEX, |history| scope.sampled(history.samples)),
                state_history: history.map_or(NONE_INDEX, |history| scope.sampled(history.states)),
                temporal: u32::from(history.is_some()),
                _pad: 0,
            },
            groups(extent, TILE),
        )
    });
    let spatial = illumination.resampling.spatial();
    let mut pass = graph
        .compute("rikka.ray.illumination.resample")
        .read_all(scene.scene_reads())
        .read(guides.half)
        .read(current.samples)
        .read(current.states)
        .write(target);
    if spatial {
        pass = pass.buffer(Buffer::Scene, traced());
    }
    pass.record(move |record, ctx, scope| {
        record.dispatch_kernel(
            ctx,
            shaders_3d::RAY_RESAMPLE_COMP,
            &ResamplePush {
                frame: scope.frame(),
                target: scope.storage(target),
                guide: scope.sampled(guides.half),
                samples: scope.sampled(current.samples),
                states: scope.sampled(current.states),
                size: [extent.width, extent.height],
                reach: illumination.reach.max(0.01),
                spatial: u32::from(spatial),
            },
            groups(extent, TILE),
        )
    });
}
