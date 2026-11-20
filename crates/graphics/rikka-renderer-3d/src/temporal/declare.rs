use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::composite::Tone;
use crate::draw::ToneMapping;
use crate::gpu::NONE_INDEX;
use crate::graph::{FrameGraph, ImageId, ImageSpec, Size, groups};
use crate::record::{Chain, PostSurface, Record, SceneImages, Stage, Written};
use crate::temporal::TemporalFrame;
use crate::viewport::COLOR_FORMAT;
use rikka_spirv::Shader;

pub(crate) const KERNELS: [Shader; 2] = [
    shaders_3d::TEMPORAL_RESOLVE_COMP,
    shaders_3d::TEMPORAL_SHARPEN_COMP,
];

const TILE: u32 = 8;
const DEPTH_FORMAT: vk::Format = vk::Format::R32_SFLOAT;

const RESET: u32 = 1 << 0;
const SETTLED: u32 = 1 << 1;
const UPSCALED: u32 = 1 << 2;
const STILL: u32 = 1 << 3;

const LINEAR_KNEE: f32 = 0.25;
const FILMIC_KNEE: f32 = 1.0;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ResolvePush {
    frame: u64,
    color: u32,
    depth: u32,
    surface: u32,
    shaded: u32,
    opaque: u32,
    history: u32,
    history_meta: u32,
    previous_depth: u32,
    output: u32,
    output_meta: u32,
    post_depth: u32,
    post_motion: u32,
    render: [u32; 2],
    display: [u32; 2],
    sample: [f32; 2],
    limit: f32,
    flags: u32,
    metered: u32,
    exposure: f32,
    knee: f32,
    blended: u32,
    emitter_motion: u32,
    pad: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct Tonal {
    metered: Option<ImageId>,
    exposure: f32,
    knee: f32,
}

impl Tonal {
    pub(crate) fn fixed(exposure: f32, tone: ToneMapping) -> Self {
        Self {
            metered: None,
            exposure,
            knee: match tone {
                ToneMapping::Clamp => LINEAR_KNEE,
                ToneMapping::Aces => FILMIC_KNEE,
            },
        }
    }

    pub(crate) fn metered(state: ImageId, exposure: f32, tone: Option<Tone>) -> Self {
        Self {
            metered: Some(state),
            ..Self::graded(exposure, tone)
        }
    }

    pub(crate) fn graded(exposure: f32, tone: Option<Tone>) -> Self {
        Self {
            metered: None,
            exposure,
            knee: match tone {
                None | Some(Tone::Clamp) => LINEAR_KNEE,
                Some(_) => FILMIC_KNEE,
            },
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SharpenPush {
    source: u32,
    destination: u32,
    size: [u32; 2],
    sharpness: f32,
    pad: u32,
}

#[derive(Clone, Copy)]
struct TemporalInputs {
    color: ImageId,
    depth: ImageId,
    surface: ImageId,
    shaded: ImageId,
    reactive: Option<(ImageId, ImageId)>,
    previous_depth: Option<ImageId>,
    emitter_motion: Option<ImageId>,
}

#[derive(Clone, Copy)]
struct Histories {
    color: ImageId,
    meta: ImageId,
    previous_color: ImageId,
    previous_meta: ImageId,
}

fn histories(graph: &mut FrameGraph<'_>, display: vk::Extent2D) -> Histories {
    let spec = |name: &'static str| {
        ImageSpec::new(
            name,
            COLOR_FORMAT,
            Size::Fixed(display.width, display.height),
        )
    };
    Histories {
        color: graph.create(spec("rikka.three.temporal.color").current(0)),
        meta: graph.create(spec("rikka.three.temporal.meta").current(0)),
        previous_color: graph.create(spec("rikka.three.temporal.color").previous(0)),
        previous_meta: graph.create(spec("rikka.three.temporal.meta").previous(0)),
    }
}

impl Record<'_> {
    pub(crate) fn declare_temporal<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        scene: &mut SceneImages,
        chain: &mut Chain,
        tonal: Tonal,
    ) {
        let Some(temporal) = stage.temporal else {
            return;
        };
        let (Some(depth), Some(surface)) = (scene.scene_depth, scene.surface) else {
            return;
        };
        let inputs = TemporalInputs {
            color: chain.current(),
            depth,
            surface,
            shaded: scene.depth_resolved,
            reactive: scene.opaque.zip(scene.blended),
            previous_depth: scene.previous_depth,
            emitter_motion: scene.emitter_motion,
        };
        let display = temporal.display;
        let kept = histories(graph, display);
        let post = temporal.upscaled.then(|| {
            let size = Size::Fixed(display.width, display.height);
            PostSurface {
                depth: graph.create(
                    ImageSpec::new("rikka.three.temporal.post_depth", DEPTH_FORMAT, size).nearest(),
                ),
                motion: graph.create(
                    ImageSpec::new("rikka.three.temporal.post_motion", COLOR_FORMAT, size)
                        .nearest(),
                ),
                ratio: temporal.ratio().to_array(),
            }
        });
        let mut pass = graph
            .compute("rikka.three.temporal.resolve")
            .read(inputs.color)
            .read(inputs.depth)
            .read(inputs.surface)
            .read(inputs.shaded)
            .read_all(
                inputs
                    .reactive
                    .into_iter()
                    .flat_map(|(opaque, blended)| [opaque, blended]),
            )
            .read_all(inputs.previous_depth)
            .read_all(inputs.emitter_motion)
            .read(kept.previous_color)
            .read(kept.previous_meta)
            .read_all(tonal.metered)
            .write(kept.color)
            .write(kept.meta);
        if let Some(post) = post {
            pass = pass.write(post.depth).write(post.motion);
        }
        pass.record(move |record, ctx, scope| {
            let slot =
                |image: Option<ImageId>| image.map_or(NONE_INDEX, |image| scope.sampled(image));
            let storage =
                |image: Option<ImageId>| image.map_or(NONE_INDEX, |image| scope.storage(image));
            let fresh = scope.fresh(kept.previous_color) || scope.fresh(kept.previous_meta);
            let previous_depth = inputs
                .previous_depth
                .filter(|&previous| !scope.fresh(previous));
            let metered = tonal.metered.filter(|&state| !scope.fresh(state));
            let push = ResolvePush {
                frame: scope.frame(),
                color: scope.sampled(inputs.color),
                depth: scope.sampled(inputs.depth),
                surface: scope.sampled(inputs.surface),
                shaded: scope.sampled(inputs.shaded),
                opaque: slot(inputs.reactive.map(|(opaque, _)| opaque)),
                history: scope.sampled(kept.previous_color),
                history_meta: scope.sampled(kept.previous_meta),
                previous_depth: slot(previous_depth),
                output: scope.storage(kept.color),
                output_meta: scope.storage(kept.meta),
                post_depth: storage(post.map(|post| post.depth)),
                post_motion: storage(post.map(|post| post.motion)),
                render: [temporal.render.width, temporal.render.height],
                display: [display.width, display.height],
                sample: (-temporal.jitter).to_array(),
                limit: temporal.limit,
                flags: flags(temporal, fresh),
                metered: slot(metered),
                exposure: tonal.exposure,
                knee: tonal.knee,
                blended: slot(inputs.reactive.map(|(_, blended)| blended)),
                emitter_motion: slot(inputs.emitter_motion),
                pad: 0,
            };
            record.dispatch_kernel(
                ctx,
                shaders_3d::TEMPORAL_RESOLVE_COMP,
                &push,
                groups(display, TILE),
            )
        });
        chain.link_to(kept.color);
        if temporal.upscaled {
            graph.enter_display(display);
            scene.post = post;
        }
        if temporal.sharpness > 0.0 {
            declare_sharpen(graph, chain, temporal.sharpness);
        }
    }
}

fn flags(temporal: TemporalFrame, fresh: bool) -> u32 {
    let mut flags = 0;
    if temporal.reset || fresh {
        flags |= RESET;
    }
    if temporal.settled {
        flags |= SETTLED;
    }
    if temporal.still {
        flags |= STILL;
    }
    if temporal.upscaled {
        flags |= UPSCALED;
    }
    flags
}

fn declare_sharpen(graph: &mut FrameGraph<'_>, chain: &mut Chain, sharpness: f32) {
    chain.filter_compute(
        graph,
        "rikka.three.temporal.sharpen",
        [],
        move |record, ctx, _, written: Written| {
            let push = SharpenPush {
                source: written.input,
                destination: written.storage,
                size: [written.extent.width, written.extent.height],
                sharpness,
                pad: 0,
            };
            record.dispatch_kernel(
                ctx,
                shaders_3d::TEMPORAL_SHARPEN_COMP,
                &push,
                groups(written.extent, TILE),
            )
        },
    );
}
