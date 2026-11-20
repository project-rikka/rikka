use bytemuck::{Pod, Zeroable};

use crate::gpu::NONE_INDEX;
use crate::graph::FrameGraph;
use crate::pipeline::{DrawState, PassFormats, Raster};
use crate::recipes;
use crate::record::stage::{SceneImages, Stage};
use crate::record::{Chain, Record};
use crate::viewport::COLOR_FORMAT;

const WORKBENCH: &str = "rikka.three.workbench";

const FLAG_CAVITY: u32 = 1;
const FLAG_CURVATURE: u32 = 2;
const FLAG_OUTLINE: u32 = 4;
const CURVATURE_RIDGE: f32 = 0.5;
const CURVATURE_VALLEY: f32 = 0.7;
const CURVATURE_FLOOR: f32 = 1e-4;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WorkbenchPush {
    frame: u64,
    color: u32,
    depth: u32,
    surface: u32,
    identity: u32,
    flags: u32,
    samples: u32,
    cavity_valley: f32,
    cavity_ridge: f32,
    cavity_distance: f32,
    cavity_attenuation: f32,
    curvature_ridge: f32,
    curvature_valley: f32,
    scale: f32,
    outline: [f32; 3],
    seed: u32,
    pad: u32,
}

impl Record<'_> {
    pub(super) fn declare_workbench<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        scene: &SceneImages,
        chain: &mut Chain,
    ) {
        let effects = stage.frame.editor.workbench;
        let (Some(depth), Some(surface)) = (scene.scene_depth, scene.surface) else {
            return;
        };
        let identity = scene.identity;
        let mut flags = 0;
        if effects.cavity.is_some() {
            flags |= FLAG_CAVITY;
        }
        if identity.is_some() {
            if effects.curvature.is_some() {
                flags |= FLAG_CURVATURE;
            }
            if effects.outline.is_some() {
                flags |= FLAG_OUTLINE;
            }
        }
        if flags == 0 {
            return;
        }
        let cavity = effects.cavity.unwrap_or_default();
        let curvature = effects.curvature.unwrap_or_default();
        let control = |factor: f32, base: f32| base / (factor * factor).max(CURVATURE_FLOOR);
        let outline = effects.outline.unwrap_or_default().to_array();
        let seed = stage.temporal.map_or(0, |temporal| {
            temporal.jitter.x.to_bits() ^ temporal.jitter.y.to_bits().rotate_left(16)
        });
        let reads = [Some(depth), Some(surface), identity];
        chain.filter(
            graph,
            WORKBENCH,
            reads.into_iter().flatten(),
            move |record, ctx, scope, color| {
                let push = WorkbenchPush {
                    frame: scope.addresses.refs,
                    color,
                    depth: scope.sampled(depth),
                    surface: scope.sampled(surface),
                    identity: identity.map_or(NONE_INDEX, |identity| scope.sampled(identity)),
                    flags,
                    samples: cavity.samples,
                    cavity_valley: cavity.valley,
                    cavity_ridge: cavity.ridge,
                    cavity_distance: cavity.distance,
                    cavity_attenuation: cavity.attenuation,
                    curvature_ridge: control(curvature.ridge, CURVATURE_RIDGE),
                    curvature_valley: control(curvature.valley, CURVATURE_VALLEY),
                    scale: effects.pixel_size.max(1.0),
                    outline,
                    seed,
                    pad: 0,
                };
                record.fullscreen(
                    ctx,
                    &recipes::fullscreen(
                        &record.mesh.builtin,
                        &record.mesh.builtin.workbench_frag,
                        Raster::FILL,
                        PassFormats::color(
                            COLOR_FORMAT,
                            ash::vk::Format::UNDEFINED,
                            ash::vk::SampleCountFlags::TYPE_1,
                        ),
                    ),
                    DrawState::NO_DEPTH,
                    bytemuck::bytes_of(&push),
                )
            },
        );
    }
}
