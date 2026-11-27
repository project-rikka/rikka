use animate_stage::PropFrame;
use animate_ui::disc;
use rikka_app::prelude::*;

use crate::prop::gizmo::PropGizmo;
use crate::prop::rig::{Part, Rig};
use crate::strokes::{line, polyline};

const AXIS_TINTS: [Color; 3] = [
    Color::rgb(0.95, 0.35, 0.38),
    Color::rgb(0.45, 0.85, 0.35),
    Color::rgb(0.35, 0.55, 1.0),
];
const FREE_TINT: Color = Color::rgb(0.95, 0.95, 0.95);
const IDLE: f32 = 0.7;
const TIP: f32 = 4.0;
const TIP_LIT: f32 = 5.5;
const CENTRE: f32 = 5.0;
const OUTLINE: f32 = 1.5;
const HALO: f32 = 1.5;

pub(crate) fn paint(ctx: &mut PaintCtx<'_>, frame: &PropFrame, rect: Rect, gizmo: &PropGizmo) {
    let rig = Rig::new(frame, rect);
    let lit = gizmo.lit();
    let tint = |part: Part, base: Color| match lit == Some(part) {
        true => base,
        false => base.scale_alpha(IDLE),
    };
    ctx.clipped(rect, |ctx| {
        for (axis, base) in AXIS_TINTS.into_iter().enumerate() {
            polyline(ctx, &rig.lens, rig.ring(axis), tint(Part::Ring(axis), base));
            let corners = rig.plane_corners(axis);
            let points = corners.iter().chain(corners.first()).copied();
            polyline(ctx, &rig.lens, points, tint(Part::Plane(axis), base));
        }
        let Some(centre) = rig.lens.project(rig.origin) else {
            return;
        };
        for (axis, base) in AXIS_TINTS.into_iter().enumerate() {
            let Some(end) = rig.lens.project(rig.arm(axis)) else {
                continue;
            };
            let colour = tint(Part::Axis(axis), base);
            line(ctx, centre, end, colour);
            let size = match lit == Some(Part::Axis(axis)) {
                true => TIP_LIT,
                false => TIP,
            };
            ctx.draw_quad(disc(end, size).fill(colour));
        }
        ctx.draw_quad(disc(centre, CENTRE + HALO).fill(Color::BLACK.scale_alpha(0.35)));
        ctx.draw_quad(
            disc(centre, CENTRE)
                .fill(tint(Part::Free, FREE_TINT).scale_alpha(0.6))
                .stroke(OUTLINE, tint(Part::Free, FREE_TINT)),
        );
    });
}
