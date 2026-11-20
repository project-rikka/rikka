use animate_stage::{EmitterMarks, Lens};
use animate_ui::disc;
use rikka_app::prelude::*;
use rikka_model::glam::{Mat4, Vec3};
use rikka_renderer_3d::EmitterShape;
use rikka_shader_graph::NodeId;

use crate::easy::Follow;
use crate::strokes::{RING_POINTS, line, polyline};
use crate::{Gesture, Marked};

const REACH: f32 = 12.0;
const TINT: Color = Color::rgb(0.78, 0.6, 1.0);
const HANDLE: f32 = 7.0;
const HANDLE_LIT: f32 = 1.3;
const OUTLINE: f32 = 1.5;
const HALO: f32 = 1.5;
const POINT_ARM: f32 = 0.08;
const CONE_LENGTH: f32 = 0.3;
const CONE_EDGES: usize = 4;
const STEEPEST_CONE: f32 = 1.4;

#[derive(Debug)]
struct Drag {
    node: NodeId,
    follow: Follow,
    start: Vec3,
    offset: Vec3,
}

#[derive(Debug, Default)]
pub(crate) struct EmitterGizmo {
    drag: Option<Drag>,
    lit: bool,
}

fn handle_at(marks: &EmitterMarks, node: NodeId, frame: Rect, at: Point) -> Option<(Vec3, Point)> {
    let centre = marks.of(&[node])?.transform.w_axis.truncate();
    let spot = Lens::new(marks.view_projection, frame).project(centre)?;
    (spot.distance(at) <= REACH).then_some((centre, spot))
}

impl EmitterGizmo {
    pub(crate) fn press(
        &mut self,
        marks: &EmitterMarks,
        node: NodeId,
        frame: Rect,
        at: Point,
    ) -> bool {
        let Some((centre, spot)) = handle_at(marks, node, frame, at) else {
            return false;
        };
        let lens = Lens::new(marks.view_projection, frame);
        self.drag = Some(Drag {
            node,
            follow: Follow::new(&lens, centre, spot, at),
            start: centre,
            offset: Vec3::ZERO,
        });
        true
    }

    pub(crate) fn travel(&mut self, marks: &EmitterMarks, frame: Rect, at: Point) {
        if let Some(drag) = &mut self.drag {
            let lens = Lens::new(marks.view_projection, frame);
            drag.offset = drag.follow.advance(&lens, at) - drag.start;
        }
    }

    pub(crate) fn release(&mut self) -> Option<Gesture> {
        let drag = self.drag.take()?;
        (drag.offset != Vec3::ZERO).then_some(Gesture::Place(drag.node, drag.offset))
    }

    pub(crate) fn cancel(&mut self) -> bool {
        self.drag.take().is_some()
    }

    pub(crate) fn hover(
        &mut self,
        marks: Option<&EmitterMarks>,
        node: Option<NodeId>,
        frame: Rect,
        at: Option<Point>,
    ) -> bool {
        let lit = match (marks, node, at) {
            (Some(marks), Some(node), Some(at)) => handle_at(marks, node, frame, at).is_some(),
            _ => false,
        };
        std::mem::replace(&mut self.lit, lit) != lit
    }

    pub(crate) fn paint(
        &self,
        ctx: &mut PaintCtx<'_>,
        marks: &EmitterMarks,
        marked: Marked,
        frame: Rect,
    ) {
        let node = marked.node;
        let Some(mark) = marks.of(&[node]) else {
            return;
        };
        let lens = Lens::new(marks.view_projection, frame);
        let shift = self
            .drag
            .as_ref()
            .filter(|drag| drag.node == node)
            .map_or(Vec3::ZERO, |drag| drag.offset);
        let transform = Mat4::from_translation(shift) * mark.transform;
        let lit = self.lit || self.drag.is_some();
        ctx.clipped(frame, |ctx| {
            outline(ctx, &lens, transform, mark.shape);
            if marked.movable {
                handle(ctx, &lens, transform.w_axis.truncate(), lit);
            }
        });
    }
}

fn handle(ctx: &mut PaintCtx<'_>, lens: &Lens, centre: Vec3, lit: bool) {
    let Some(at) = lens.project(centre) else {
        return;
    };
    let size = match lit {
        true => HANDLE * HANDLE_LIT,
        false => HANDLE,
    };
    ctx.draw_quad(disc(at, size + HALO).fill(Color::BLACK.scale_alpha(0.35)));
    let fill = match lit {
        true => TINT.scale_alpha(0.9),
        false => TINT.scale_alpha(0.5),
    };
    ctx.draw_quad(disc(at, size).fill(fill).stroke(OUTLINE, TINT));
}

fn circle(transform: Mat4, centre: Vec3, radius: f32) -> impl Iterator<Item = Vec3> {
    (0..=RING_POINTS).map(move |step| {
        let turn = std::f32::consts::TAU * step as f32 / RING_POINTS as f32;
        transform.transform_point3(centre + Vec3::new(turn.cos(), 0.0, turn.sin()) * radius)
    })
}

fn outline(ctx: &mut PaintCtx<'_>, lens: &Lens, transform: Mat4, shape: EmitterShape) {
    let place = |point: Vec3| transform.transform_point3(point);
    match shape {
        EmitterShape::Point => {
            for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                let arm = axis * POINT_ARM;
                polyline(ctx, lens, [place(-arm), place(arm)], TINT);
            }
        }
        EmitterShape::Sphere { radius } => {
            for turned in [
                Mat4::IDENTITY,
                Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2),
                Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2),
            ] {
                polyline(
                    ctx,
                    lens,
                    circle(transform * turned, Vec3::ZERO, radius),
                    TINT,
                );
            }
        }
        EmitterShape::Box { half_extents } => {
            let corner = |index: usize| {
                let sign = |bit: usize| match index & (1 << bit) {
                    0 => -1.0,
                    _ => 1.0,
                };
                place(half_extents * Vec3::new(sign(0), sign(1), sign(2)))
            };
            for from in 0..8 {
                for bit in 0..3 {
                    let to = from | (1 << bit);
                    if to != from {
                        polyline(ctx, lens, [corner(from), corner(to)], TINT);
                    }
                }
            }
        }
        EmitterShape::Disc { radius } => {
            polyline(ctx, lens, circle(transform, Vec3::ZERO, radius), TINT);
        }
        EmitterShape::Cone { radius, angle } => {
            let rim = Vec3::Y * CONE_LENGTH;
            let reach = radius + CONE_LENGTH * angle.clamp(0.0, STEEPEST_CONE).tan();
            polyline(ctx, lens, circle(transform, Vec3::ZERO, radius), TINT);
            polyline(ctx, lens, circle(transform, rim, reach), TINT);
            for edge in 0..CONE_EDGES {
                let turn = std::f32::consts::TAU * edge as f32 / CONE_EDGES as f32;
                let out = Vec3::new(turn.cos(), 0.0, turn.sin());
                let (Some(from), Some(to)) = (
                    lens.project(place(out * radius)),
                    lens.project(place(rim + out * reach)),
                ) else {
                    continue;
                };
                line(ctx, from, to, TINT.scale_alpha(0.7));
            }
        }
        EmitterShape::Line { length } => {
            let half = Vec3::X * (length * 0.5);
            polyline(ctx, lens, [place(-half), place(half)], TINT);
        }
    }
}
