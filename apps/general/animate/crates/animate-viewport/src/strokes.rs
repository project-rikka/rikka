use animate_stage::Lens;
use rikka_app::prelude::*;
use rikka_model::glam::Vec3;

const LINE: f32 = 1.4;
const LINE_STEP: f32 = 0.7;
pub(crate) const RING_POINTS: usize = 32;

pub(crate) fn basis(axis: Vec3) -> (Vec3, Vec3) {
    let across = axis.any_orthonormal_vector();
    (across, axis.cross(across))
}

pub(crate) fn ring(
    ctx: &mut PaintCtx<'_>,
    lens: &Lens,
    centre: Vec3,
    axis: Vec3,
    radius: f32,
    tint: Color,
) {
    let (across, beside) = basis(axis.normalize_or(Vec3::Y));
    let points = (0..=RING_POINTS).map(|step| {
        let turn = std::f32::consts::TAU * step as f32 / RING_POINTS as f32;
        centre + (across * turn.cos() + beside * turn.sin()) * radius
    });
    polyline(ctx, lens, points, tint);
}

pub(crate) fn polyline(
    ctx: &mut PaintCtx<'_>,
    lens: &Lens,
    points: impl IntoIterator<Item = Vec3>,
    tint: Color,
) {
    let mut previous: Option<Point> = None;
    for point in points {
        let projected = lens.project(point);
        if let (Some(from), Some(to)) = (previous, projected) {
            line(ctx, from, to, tint);
        }
        previous = projected;
    }
}

pub(crate) fn line(ctx: &mut PaintCtx<'_>, from: Point, to: Point, tint: Color) {
    let step = (LINE * LINE_STEP).max(0.5);
    let steps = (from.distance(to) / step).ceil().max(1.0) as usize;
    for at in 0..=steps {
        let along = at as f32 / steps as f32;
        ctx.draw_circle(from + (to - from) * along, LINE * 0.5, tint);
    }
}
