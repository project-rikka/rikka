use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::composite::{Exposure, Metering};
use crate::gpu::NONE_INDEX;
use crate::graph::{FrameGraph, ImageId, ImageSpec, Size};
use crate::post::Exposed;
use crate::post::kernel::{covering, size_of_extent};
use crate::record::{Chain, SceneImages, Stage, Written};

const STATE: Size = Size::Fixed(4, 1);

const DAYLIGHT_EV: f32 = -1.7;
const NIGHT_COMPENSATION: f32 = 0.5;
const RISE: f32 = 1.2;
const FALL: f32 = 2.5;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MeterPush {
    source: u32,
    state: u32,
    snap: u32,
    time: f32,
    adaptation: f32,
    compensation: f32,
    reference: f32,
    night: f32,
    darkest: f32,
    brightest: f32,
    rise: f32,
    fall: f32,
    surface: u32,
    figure_weight: f32,
    surface_size: [u32; 2],
    low_percentile: f32,
    high_percentile: f32,
    center_weight: f32,
    pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ExposePush {
    source: u32,
    destination: u32,
    state: u32,
    exposure: f32,
    size: [u32; 2],
}

pub(crate) fn state(graph: &mut FrameGraph<'_>, key: u64) -> ImageId {
    graph.create(
        ImageSpec::new("rikka.three.post.exposure", vk::Format::R32_SFLOAT, STATE)
            .nearest()
            .history(key),
    )
}

pub(crate) fn fixed(exposure: &Exposure, frame_exposure: f32) -> Option<f32> {
    match exposure.metering {
        Metering::Auto => None,
        Metering::Manual => Some(exposure.compensation.exp2() * frame_exposure),
        Metering::Physical => Some(exposure.physical()),
    }
}

pub(crate) fn declare<'f>(
    graph: &mut FrameGraph<'f>,
    stage: Stage<'f>,
    scene: &SceneImages,
    input: ImageId,
    exposure: &'f Exposure,
    exposed: Exposed,
) -> ImageId {
    if let Exposed::Metered(state) = exposed {
        meter(graph, stage, scene.surface, input, exposure, state);
    }
    let mut chain = Chain::start(input);
    let state = Exposed::image(Some(exposed));
    chain.filter_compute(
        graph,
        "rikka.three.post.expose",
        state,
        move |record, ctx, scope, written: Written| {
            let push = ExposePush {
                source: written.input,
                destination: written.storage,
                state: state.map_or(NONE_INDEX, |state| scope.sampled(state)),
                exposure: match exposed {
                    Exposed::Fixed(value) => value,
                    Exposed::Metered(_) => 1.0,
                },
                size: size_of_extent(written.extent),
            };
            record.dispatch_kernel(
                ctx,
                shaders_3d::POST_EXPOSE_COMP,
                &push,
                covering(written.extent),
            )
        },
    );
    chain.current()
}

fn meter<'f>(
    graph: &mut FrameGraph<'f>,
    stage: Stage<'f>,
    surface: Option<ImageId>,
    source: ImageId,
    exposure: &'f Exposure,
    state: ImageId,
) {
    let frame = stage.frame;
    graph
        .compute("rikka.three.post.exposure")
        .read(source)
        .modify(state)
        .read_all(surface)
        .record(move |record, ctx, scope| {
            let push = MeterPush {
                source: scope.sampled(source),
                state: scope.storage(state),
                snap: u32::from(scope.fresh(state)),
                time: frame.time,
                adaptation: exposure.adaptation.clamp(0.0, 1.0),
                compensation: exposure.compensation + frame.exposure.max(1e-6).log2(),
                reference: DAYLIGHT_EV,
                night: NIGHT_COMPENSATION,
                darkest: exposure.darkest.min(exposure.brightest),
                brightest: exposure.brightest.max(exposure.darkest),
                rise: RISE,
                fall: FALL,
                surface: surface.map_or(NONE_INDEX, |surface| scope.sampled(surface)),
                figure_weight: exposure.figure_metering.clamp(0.0, 1.0),
                surface_size: surface.map_or([1; 2], |surface| {
                    let extent = scope.extent(surface);
                    [extent.width, extent.height]
                }),
                low_percentile: exposure.low_percentile.clamp(0.0, 1.0),
                high_percentile: exposure
                    .high_percentile
                    .clamp(exposure.low_percentile.clamp(0.0, 1.0), 1.0),
                center_weight: exposure.center_weight.clamp(0.0, 1.0),
                pad: 0,
            };
            record.dispatch_kernel(ctx, shaders_3d::POST_EXPOSURE_COMP, &push, (1, 1))
        });
}
