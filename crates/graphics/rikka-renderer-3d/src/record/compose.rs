use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2, Vec3};

use crate::composite::{self, NodeOutput, Operation};
use crate::draw::{Frame, ToneMapping};
use crate::gpu::{GpuInstance, NONE_INDEX, instance_flags};
use crate::graph::{Attachment, FrameGraph, Load};
use crate::pipeline::DrawState;
use crate::post::{exposure, finish};
use crate::recipes;
use crate::record::stage::{SceneImages, Stage};
use crate::record::{Chain, Record};
use crate::temporal::Tonal;

const OUTLINE_LIMIT: u32 = 16;

const TONE_ACES: u32 = 0;
const TONE_CLAMP: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TonemapPush {
    source: u32,
    exposure: f32,
    curve: u32,
    outline: u32,
    reach: u32,
    color: [f32; 4],
    area: [i32; 4],
    ratio: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct OutlinePush {
    identity: u32,
    reach: u32,
    size: [u32; 2],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct OutlineMask {
    pub area: vk::Rect2D,
    pub reach: u32,
    pub ratio: [f32; 2],
}

impl Record<'_> {
    pub(super) fn declare_compose<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        scene: &mut SceneImages,
        mut chain: Chain,
        outputs: &mut Vec<Option<NodeOutput>>,
    ) {
        let outlined = declare_outline(graph, stage, scene);
        let Some(look) = stage.look else {
            let tonal = Tonal::fixed(stage.frame.exposure, stage.frame.tone_mapping);
            self.declare_temporal(graph, stage, scene, &mut chain, tonal);
            declare_tonemap(graph, stage, scene, &chain, outlined);
            self.declare_overlays(graph, stage, scene);
            return;
        };
        self.declare_reflections(graph, stage, look, scene, &mut chain);
        self.declare_lines(graph, look, scene, &mut chain);
        self.declare_fog(graph, look, scene, &mut chain);
        let plan = stage.composite;
        let frame_exposure = stage.frame.exposure;
        let first = plan
            .first_exposure()
            .map(|index| &look.composite.nodes[index]);
        let (first_state, tonal) = match first.map(|node| (node, &node.operation)) {
            Some((node, Operation::Exposure(settings))) => {
                match exposure::fixed(settings, frame_exposure) {
                    Some(value) => (None, Tonal::graded(value, plan.tone())),
                    None => {
                        let state = exposure::state(graph, node.key);
                        (
                            Some(state),
                            Tonal::metered(state, frame_exposure, plan.tone()),
                        )
                    }
                }
            }
            _ => (None, Tonal::graded(frame_exposure, plan.tone())),
        };
        self.declare_temporal(graph, stage, scene, &mut chain, tonal);
        let scene = &*scene;
        let output = composite::declare(graph, stage, scene, chain.current(), first_state, outputs);
        if plan.reads().previous {
            composite::keep_output(graph, output);
        }
        finish::present(graph, stage, scene, output, outlined);
        self.declare_overlays(graph, stage, scene);
    }
}

fn declare_tonemap<'f>(
    graph: &mut FrameGraph<'f>,
    stage: Stage<'f>,
    scene: &SceneImages,
    chain: &Chain,
    outlined: Option<OutlineMask>,
) {
    let source = chain.current();
    let frame = stage.frame;
    let outline_slot = stage.view.outline_slot;
    graph
        .render("rikka.three.tonemap")
        .color(Attachment::new(scene.output, Load::Discard))
        .read(source)
        .read_all(outlined.and(scene.outline))
        .record(move |record, ctx, scope| {
            let area = outlined.map_or([0; 4], |outlined| {
                let rect = outlined.area;
                [
                    rect.offset.x,
                    rect.offset.y,
                    rect.offset.x + rect.extent.width as i32,
                    rect.offset.y + rect.extent.height as i32,
                ]
            });
            let push = TonemapPush {
                source: scope.sampled(source),
                exposure: frame.exposure,
                curve: match frame.tone_mapping {
                    ToneMapping::Aces => TONE_ACES,
                    ToneMapping::Clamp => TONE_CLAMP,
                },
                outline: match outlined {
                    Some(_) => outline_slot,
                    None => NONE_INDEX,
                },
                reach: outlined.map_or(0, |outlined| outlined.reach),
                color: frame.outline.color,
                area,
                ratio: outlined.map_or([1.0; 2], |outlined| outlined.ratio),
            };
            record.fullscreen(
                ctx,
                &recipes::tonemap(&record.mesh.builtin),
                DrawState::NO_DEPTH,
                bytemuck::bytes_of(&push),
            )
        });
}

fn declare_outline(
    graph: &mut FrameGraph<'_>,
    stage: Stage<'_>,
    scene: &SceneImages,
) -> Option<OutlineMask> {
    let frame: &Frame = stage.frame;
    let (Some(image), Some(identity)) = (scene.outline, scene.identity) else {
        return None;
    };
    if !frame.outline.enabled {
        return None;
    }
    let extent = stage.view.extent;
    let width = frame.outline.width.clamp(1, OUTLINE_LIMIT);
    let selected = stage.built.instances.iter().filter(|instance| {
        instance.index_count > 0
            && instance.flags & instance_flags::SELECTED != 0
            && instance.flags & instance_flags::DIRECT == 0
    });
    let area = screen_area(frame.camera.view_projection(), selected, extent, width + 1)?;
    let identity_slot = stage.view.identity_slot;
    let reach = width - 1;
    graph
        .render("rikka.three.outline")
        .color(Attachment::new(image, Load::Discard))
        .area(area)
        .read(identity)
        .record(move |record, ctx, _| {
            record.fullscreen(
                ctx,
                &recipes::outline(&record.mesh.builtin),
                DrawState::NO_DEPTH,
                bytemuck::bytes_of(&OutlinePush {
                    identity: identity_slot,
                    reach,
                    size: [extent.width, extent.height],
                }),
            )
        });
    let ratio = stage
        .temporal
        .map_or([1.0; 2], |temporal| temporal.ratio().to_array());
    Some(OutlineMask { area, reach, ratio })
}

pub(super) fn screen_area<'i>(
    view_projection: Mat4,
    instances: impl IntoIterator<Item = &'i GpuInstance>,
    extent: vk::Extent2D,
    margin: u32,
) -> Option<vk::Rect2D> {
    let size = Vec2::new(extent.width as f32, extent.height as f32);
    let mut bounds: Option<(Vec2, Vec2)> = None;
    for instance in instances {
        let center = Vec3::from_array(instance.center);
        let (low, high) = screen_bounds(view_projection, center, instance.radius, size);
        bounds = Some(match bounds {
            Some((a, b)) => (a.min(low), b.max(high)),
            None => (low, high),
        });
    }
    let (low, high) = bounds?;
    let margin = margin as f32;
    let low = (low - margin).floor().max(Vec2::ZERO);
    let high = (high + margin).ceil().min(size);
    if low.x >= high.x || low.y >= high.y {
        return None;
    }
    Some(vk::Rect2D {
        offset: vk::Offset2D {
            x: low.x as i32,
            y: low.y as i32,
        },
        extent: vk::Extent2D {
            width: (high.x - low.x) as u32,
            height: (high.y - low.y) as u32,
        },
    })
}

fn screen_bounds(view_projection: Mat4, center: Vec3, radius: f32, size: Vec2) -> (Vec2, Vec2) {
    if !radius.is_finite() {
        return (Vec2::ZERO, size);
    }
    let mut low = Vec2::splat(f32::MAX);
    let mut high = Vec2::splat(f32::MIN);
    for corner in 0..8 {
        let sign = |bit: u32| match corner & bit {
            0 => -1.0,
            _ => 1.0,
        };
        let point = center + radius * Vec3::new(sign(1), sign(2), sign(4));
        let clip = view_projection * point.extend(1.0);
        if clip.w <= f32::EPSILON {
            return (Vec2::ZERO, size);
        }
        let pixel = (clip.truncate().truncate() / clip.w * 0.5 + 0.5) * size;
        low = low.min(pixel);
        high = high.max(pixel);
    }
    (low, high)
}
