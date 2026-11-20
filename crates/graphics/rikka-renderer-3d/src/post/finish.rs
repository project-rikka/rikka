use bytemuck::{Pod, Zeroable};

use crate::composite::{AgxLook, Grade, Lens, Tone, Tonemap};
use crate::gpu::NONE_INDEX;
use crate::graph::{Attachment, FrameGraph, ImageId, ImageSpec, Load, Size};
use crate::pipeline::DrawState;
use crate::post::kernel::extent_of;
use crate::post::{Exposed, Pass, white_balance};
use crate::record::{OutlineMask, Record, SceneImages, Stage};
use crate::viewport::COLOR_FORMAT;

const SEED_RATE: f32 = 60.0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuGrade {
    pub balance: [[f32; 4]; 3],
    pub lift: [f32; 3],
    pub contrast: f32,
    pub gamma: [f32; 3],
    pub saturation: f32,
    pub gain: [f32; 3],
    pub vibrance: f32,
    pub shadow_tint: [f32; 3],
    pub black: f32,
    pub highlight_tint: [f32; 3],
    pub night: f32,
    pub display: u32,
    pub pad: [u32; 3],
}

impl GpuGrade {
    pub(crate) fn of(grade: &Grade) -> Self {
        let balance = white_balance(grade.temperature, grade.tint);
        Self {
            balance: [0, 1, 2].map(|row| balance.row(row).extend(0.0).to_array()),
            lift: grade.lift.to_array(),
            contrast: grade.contrast.max(0.0),
            gamma: grade.gamma.to_array(),
            saturation: grade.saturation.max(0.0),
            gain: grade.gain.to_array(),
            vibrance: grade.vibrance,
            shadow_tint: grade.shadow_tint.to_array(),
            black: grade.black.clamp(0.0, 1.0),
            highlight_tint: grade.highlight_tint.to_array(),
            night: grade.night.clamp(0.0, 1.0),
            display: u32::from(grade.grades_display()),
            pad: [0; 3],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GradePush {
    params: u64,
    source: u32,
    exposure: u32,
    exposure_value: f32,
    pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TonemapPush {
    source: u32,
    tone: u32,
    look: u32,
    preserve_hue: f32,
    peak: f32,
    contrast: f32,
    toe: f32,
    linear: f32,
    toe_power: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LensPush {
    source: u32,
    seed: u32,
    size: [f32; 2],
    chromatic: f32,
    vignette: f32,
    grain: f32,
    sharpen: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PresentPush {
    source: u32,
    seed: u32,
    outline: u32,
    reach: u32,
    area: [i32; 4],
    outline_color: [f32; 4],
    ratio: [f32; 2],
}

fn seed(time: f32) -> u32 {
    (time * SEED_RATE).max(0.0) as u32
}

fn filter<'f, P: Pod>(
    graph: &mut FrameGraph<'f>,
    name: &'static str,
    pass: Pass,
    reads: impl IntoIterator<Item = ImageId>,
    push: impl FnOnce(&crate::graph::PassScope<'_>) -> P + 'f,
) -> ImageId {
    let target = graph.create(ImageSpec::new(name, COLOR_FORMAT, Size::Viewport));
    graph
        .render(name)
        .color(Attachment::new(target, Load::Discard))
        .read_all(reads)
        .record(move |record: &mut Record<'_>, ctx, scope| {
            let push = push(scope);
            let recipe = record
                .mesh
                .look_state
                .post
                .recipe(&record.mesh.builtin, pass);
            record.fullscreen(ctx, &recipe, DrawState::NO_DEPTH, bytemuck::bytes_of(&push))
        });
    target
}

pub(crate) fn grade(
    graph: &mut FrameGraph<'_>,
    source: ImageId,
    block: u64,
    exposed: Option<Exposed>,
) -> ImageId {
    let reads = Exposed::image(exposed).into_iter().chain([source]);
    filter(
        graph,
        "rikka.three.post.grade",
        Pass::Grade,
        reads,
        move |scope| {
            let (exposure, exposure_value) = Exposed::push(exposed, scope);
            GradePush {
                params: scope.addresses.effects + block,
                source: scope.sampled(source),
                exposure,
                exposure_value,
                pad: 0,
            }
        },
    )
}

pub(crate) fn tonemap(graph: &mut FrameGraph<'_>, source: ImageId, tonemap: &Tonemap) -> ImageId {
    let curve = tonemap.curve;
    let push = TonemapPush {
        source: 0,
        tone: match tonemap.tone {
            Tone::Gt => 0,
            Tone::Agx => 1,
            Tone::Aces => 2,
            Tone::Neutral => 3,
            Tone::Clamp => 4,
            Tone::Hable => 5,
            Tone::Reinhard => 6,
            Tone::Lottes => 7,
            Tone::Custom => 8,
        },
        look: match tonemap.look {
            AgxLook::Base => 0,
            AgxLook::Punchy => 1,
            AgxLook::Golden => 2,
        },
        preserve_hue: tonemap.preserve_hue.clamp(0.0, 1.0),
        peak: curve.peak,
        contrast: curve.contrast,
        toe: curve.toe,
        linear: curve.linear,
        toe_power: curve.toe_power,
    };
    filter(
        graph,
        "rikka.three.post.tonemap",
        Pass::Tonemap,
        [source],
        move |scope| TonemapPush {
            source: scope.sampled(source),
            ..push
        },
    )
}

pub(crate) fn lens<'f>(
    graph: &mut FrameGraph<'f>,
    stage: Stage<'f>,
    source: ImageId,
    lens: &Lens,
) -> ImageId {
    if !lens.is_active() {
        return source;
    }
    let size = extent_of(graph.extent());
    let push = LensPush {
        source: 0,
        seed: seed(stage.frame.time),
        size,
        chromatic: lens.chromatic_aberration.max(0.0),
        vignette: lens.vignette.clamp(0.0, 1.0),
        grain: lens.grain.max(0.0),
        sharpen: lens.sharpen.clamp(0.0, 1.0),
    };
    filter(
        graph,
        "rikka.three.post.lens",
        Pass::Lens,
        [source],
        move |scope| LensPush {
            source: scope.sampled(source),
            ..push
        },
    )
}

pub(crate) fn present<'f>(
    graph: &mut FrameGraph<'f>,
    stage: Stage<'f>,
    scene: &SceneImages,
    source: ImageId,
    outlined: Option<OutlineMask>,
) {
    let outline = outlined.and(scene.outline);
    let outline_slot = stage.view.outline_slot;
    let frame = stage.frame;
    graph
        .render("rikka.three.present")
        .color(Attachment::new(scene.output, Load::Discard))
        .read(source)
        .read_all(outline)
        .record(move |record, ctx, scope| {
            let mut push = PresentPush {
                source: scope.sampled(source),
                seed: seed(frame.time),
                outline: NONE_INDEX,
                reach: 0,
                area: [0; 4],
                outline_color: [0.0; 4],
                ratio: [1.0; 2],
            };
            if let Some(outlined) = outlined.filter(|_| outline.is_some()) {
                let rect = outlined.area;
                push.outline = outline_slot;
                push.reach = outlined.reach;
                push.area = [
                    rect.offset.x,
                    rect.offset.y,
                    rect.offset.x + rect.extent.width as i32,
                    rect.offset.y + rect.extent.height as i32,
                ];
                push.outline_color = frame.outline.color;
                push.ratio = outlined.ratio;
            }
            let recipe = record
                .mesh
                .look_state
                .post
                .recipe(&record.mesh.builtin, Pass::Present);
            record.fullscreen(ctx, &recipe, DrawState::NO_DEPTH, bytemuck::bytes_of(&push))
        });
}
