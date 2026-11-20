use animate_fluid::{Anchor, EmitterSetting, Shape};
use animate_stage::{Aiming, Lens, Placed};
use animate_ui::{disc, dotted};
use rikka_app::prelude::*;
use rikka_model::glam::Vec3;

use crate::fluid::gizmo::Gizmo;
use crate::strokes::{basis, line, ring};

const LIQUID: Color = Color::rgb(0.35, 0.8, 1.0);
const TARGET: Color = Color::rgb(1.0, 0.72, 0.3);
const BONE: Color = Color::rgb(0.95, 0.4, 0.85);
const PATH_DOT: f32 = 1.8;
const PATH_STEP: f32 = 5.0;
const HANDLE: f32 = 7.0;
const HANDLE_LIT: f32 = 1.3;
const OUTLINE: f32 = 1.5;
const HALO: f32 = 1.5;
const SNAP: f32 = 9.0;
const CONE_LENGTH: f32 = 0.3;
const CONE_EDGES: usize = 4;
const OFF: f32 = 0.4;

pub(crate) fn paint(ctx: &mut PaintCtx<'_>, aiming: &Aiming, frame: Rect, gizmo: &Gizmo) {
    let lens = Lens::new(aiming.view_projection, frame);
    let strength = match aiming.state.on {
        true => 1.0,
        false => OFF,
    };
    ctx.clipped(frame, |ctx| {
        let path: Vec<Point> = aiming
            .path
            .iter()
            .filter_map(|point| lens.project(*point))
            .collect();
        dots(ctx, &path, LIQUID.scale_alpha(0.85 * strength));
        if let Some(launch) = &aiming.launch {
            outline(
                ctx,
                &lens,
                aiming,
                launch.world,
                LIQUID.scale_alpha(0.7 * strength),
            );
        }
        for (setting, tint) in [
            (EmitterSetting::Target, TARGET),
            (EmitterSetting::Launch, LIQUID),
        ] {
            let Some(placed) = aiming.placed(setting) else {
                continue;
            };
            if let Some(spot) = aiming.state.spot(setting) {
                tether(ctx, &lens, &spot.anchor, placed);
            }
            handle(
                ctx,
                &lens,
                placed.world,
                tint,
                gizmo.lit() == Some(setting),
                setting,
            );
        }
        if let Some(snapped) = gizmo.snapped() {
            ctx.draw_ring(Ring::new(snapped, SNAP * 2.0).stroke(OUTLINE, BONE));
        }
    });
}

fn handle(
    ctx: &mut PaintCtx<'_>,
    lens: &Lens,
    world: Vec3,
    tint: Color,
    highlighted: bool,
    setting: EmitterSetting,
) {
    let Some(at) = lens.project(world) else {
        return;
    };
    let size = match highlighted {
        true => HANDLE * HANDLE_LIT,
        false => HANDLE,
    };
    ctx.draw_quad(disc(at, size + HALO).fill(Color::BLACK.scale_alpha(0.35)));
    let fill = match (setting, highlighted) {
        (EmitterSetting::Launch, true) => tint.scale_alpha(0.9),
        (EmitterSetting::Launch, false) => tint.scale_alpha(0.55),
        (_, true) => tint.scale_alpha(0.45),
        (_, false) => Color::TRANSPARENT,
    };
    ctx.draw_quad(disc(at, size).fill(fill).stroke(OUTLINE, tint));
    if setting == EmitterSetting::Target {
        ctx.draw_circle(at, OUTLINE, tint);
    }
}

fn tether(ctx: &mut PaintCtx<'_>, lens: &Lens, anchor: &Anchor, placed: &Placed) {
    if *anchor == Anchor::World {
        return;
    }
    let (Some(bone), Some(spot)) = (
        lens.project(placed.frame.w_axis.truncate()),
        lens.project(placed.world),
    ) else {
        return;
    };
    dotted(ctx, bone, spot, BONE);
    let mark = Rect::from_center(bone, Size::new(HANDLE, HANDLE));
    ctx.draw_quad(
        Quad::new(mark)
            .fill(BONE.scale_alpha(0.3))
            .stroke(OUTLINE, BONE)
            .radius(1.0),
    );
}

fn outline(ctx: &mut PaintCtx<'_>, lens: &Lens, aiming: &Aiming, launch: Vec3, tint: Color) {
    let heading = aiming.heading;
    let levels = aiming.state.levels;
    match aiming.state.shape {
        Shape::Stream | Shape::Drip => ring(ctx, lens, launch, heading, levels.width * 0.5, tint),
        Shape::Shower => {
            let reach = CONE_LENGTH;
            let radius = reach * (levels.spread.to_radians() * 0.5).tan();
            let rim = launch + heading * reach;
            ring(ctx, lens, rim, heading, radius, tint);
            let (across, beside) = basis(heading);
            for edge in 0..CONE_EDGES {
                let turn = std::f32::consts::TAU * edge as f32 / CONE_EDGES as f32;
                let to = rim + (across * turn.cos() + beside * turn.sin()) * radius;
                if let (Some(from), Some(to)) = (lens.project(launch), lens.project(to)) {
                    line(ctx, from, to, tint);
                }
            }
        }
        Shape::Blob { size } => {
            ring(ctx, lens, launch, Vec3::Y, size * 0.5, tint);
            ring(ctx, lens, launch, Vec3::X, size * 0.5, tint);
            ring(ctx, lens, launch, Vec3::Z, size * 0.5, tint);
        }
        Shape::Surface { .. } => {}
    }
}

fn dots(ctx: &mut PaintCtx<'_>, spots: &[Point], tint: Color) {
    let mut carried = 0.0;
    for pair in spots.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let length = from.distance(to);
        let mut along = PATH_STEP - carried;
        while along <= length {
            ctx.draw_circle(
                from + (to - from) * (along / length.max(1e-6)),
                PATH_DOT,
                tint,
            );
            along += PATH_STEP;
        }
        carried = (carried + length) % PATH_STEP;
    }
}
