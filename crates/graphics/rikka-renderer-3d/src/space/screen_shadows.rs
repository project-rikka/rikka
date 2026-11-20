use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3, Vec4};

use crate::camera::Camera;
use crate::graph::{FrameGraph, ImageSpec, Size};
use crate::record::SceneImages;

const FORMAT: vk::Format = vk::Format::R32_SFLOAT;
const WAVE: i64 = 64;
const FAR_AWAY: f32 = 1.0e5;
const PARALLEL: f32 = 1e-6;
const LIGHT_FIXED: f32 = 64.0;
const ROW_MARGIN: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MarchPush {
    frame: u64,
    target: u32,
    sector: u32,
    light: [f32; 2],
    march: f32,
    first_block: i32,
    along: [f32; 4],
    depth: [f32; 4],
    toward: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
struct Dispatch {
    sector: u32,
    first_block: i32,
    rows: u32,
    blocks: u32,
}

#[derive(Clone, Copy, Debug)]
struct Light {
    screen: Vec2,
    march: f32,
    along: Vec4,
    depth: [f32; 4],
    toward: Vec3,
}

fn light_on_screen(camera: &Camera, direction: Vec3, size: Vec2) -> Light {
    let toward = -direction.normalize_or(Vec3::NEG_Y);
    let along = camera.view.transform_vector3(toward).normalize_or(Vec3::Z);
    let projection = camera.projection;
    let clip: Vec4 = projection * along.extend(0.0);
    let depth = [
        projection.z_axis.z,
        projection.z_axis.w,
        projection.w_axis.z,
        projection.w_axis.w,
    ];
    let pixels = |ndc: Vec2| (ndc * 0.5 + 0.5) * size;
    let (screen, march) = match clip.w.abs() > PARALLEL {
        true => (pixels(Vec2::new(clip.x, clip.y) / clip.w), clip.w.signum()),
        false => {
            let across = Vec2::new(clip.x, clip.y).normalize_or(Vec2::X);
            (size * 0.5 + across * FAR_AWAY, 1.0)
        }
    };
    let centre = size * 0.5;
    let screen = centre + (screen - centre).clamp_length_max(FAR_AWAY);
    let screen = (screen * LIGHT_FIXED).round() / LIGHT_FIXED;
    Light {
        screen,
        march,
        along: clip,
        depth,
        toward,
    }
}

fn column_u(light_major: f32, side: f32, base: f32, column: i64) -> f32 {
    let centre = base + side * column as f32 + 0.5;
    (side * (centre - light_major)).max(1e-3)
}

fn row_span(light_minor: f32, extent: f32, far_u: f32, near_u: f32) -> Option<(f32, f32)> {
    let lo = -light_minor;
    let hi = extent - light_minor;
    let shrink = near_u / far_u;
    let lowest = (if lo < 0.0 { lo / shrink } else { lo }).max(-far_u);
    let highest = (if hi > 0.0 { hi / shrink } else { hi }).min(far_u);
    (lowest <= highest).then_some((lowest, highest))
}

fn dispatch(light: &Light, size: Vec2, sector: u32) -> Option<Dispatch> {
    let major = (sector >= 2) as usize;
    let side = if sector & 1 == 0 { 1.0 } else { -1.0 };
    let light_major = light.screen[major];
    let light_minor = light.screen[1 - major];
    let extent_major = size[major] as i64;
    let extent_minor = size[1 - major];
    let base = (light_major + 0.5 * side).floor();
    let base_column = base as i64;
    let (low, high) = match side > 0.0 {
        true => (-base_column, extent_major - 1 - base_column),
        false => (base_column - (extent_major - 1), base_column),
    };
    let low = low.max(0);
    if high < low {
        return None;
    }
    let (first, last) = (low.div_euclid(WAVE), high.div_euclid(WAVE));
    let mut rows = 0;
    for block in first..=last {
        let far_u = column_u(light_major, side, base, block * WAVE + WAVE - 1);
        let near_u = column_u(light_major, side, base, block * WAVE);
        if let Some((lowest, highest)) = row_span(light_minor, extent_minor, far_u, near_u) {
            let span = (light_minor + highest + 1.0).floor() - (light_minor + lowest).floor() + 1.0;
            rows = rows.max(span.max(0.0) as u32);
        }
    }
    (rows > 0).then_some(Dispatch {
        sector,
        first_block: first as i32,
        rows: rows + 2 * ROW_MARGIN,
        blocks: (last - first + 1) as u32,
    })
}

pub(super) fn declare(
    graph: &mut FrameGraph<'_>,
    scene: &mut SceneImages,
    camera: &Camera,
    leading: Vec3,
) {
    let extent = graph.extent();
    let size = Vec2::new(extent.width as f32, extent.height as f32);
    let light = light_on_screen(camera, leading, size);
    let shadow = graph.create(ImageSpec::new(
        "rikka.space.screen_shadow",
        FORMAT,
        Size::Viewport,
    ));
    graph.publish(shadow);
    graph
        .compute("rikka.three.screen_shadow")
        .read_all(scene.scene_reads())
        .write(shadow)
        .record(move |record, ctx, scope| {
            for sector in 0..4 {
                let Some(dispatch) = dispatch(&light, size, sector) else {
                    continue;
                };
                record.dispatch_kernel(
                    ctx,
                    shaders_3d::SPACE_SCREEN_SHADOW_COMP,
                    &MarchPush {
                        frame: scope.frame(),
                        target: scope.storage(shadow),
                        sector: dispatch.sector,
                        light: light.screen.to_array(),
                        march: light.march,
                        first_block: dispatch.first_block,
                        along: light.along.to_array(),
                        depth: light.depth,
                        toward: light.toward.extend(0.0).to_array(),
                    },
                    (dispatch.rows, dispatch.blocks),
                )?;
            }
            Ok(())
        });
    scene.screen_shadow = Some(shadow);
}
