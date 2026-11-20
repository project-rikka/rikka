use paint_document::{Combine, Shape};
use rikka_app::prelude::*;
use rikka_figure_editing::paint_dashed;
use rikka_image_edit::FillRule;

const CLOSE_SLOP: f32 = 6.0;
const DASH: f32 = 4.0;
const VERTEX: f32 = 6.0;
const VERTEX_CORE: f32 = 3.0;
const LIGHT: Color = Color::WHITE;
const DARK: Color = Color::BLACK;

#[derive(Debug)]
pub(crate) struct Path {
    pub(crate) combine: Combine,
    points: Vec<Point>,
    reach: Option<Point>,
    polygon: bool,
}

impl Path {
    pub(crate) fn lasso(combine: Combine, from: Point) -> Path {
        Path {
            combine,
            points: vec![from],
            reach: None,
            polygon: false,
        }
    }

    pub(crate) fn polygon(combine: Combine, from: Point) -> Path {
        Path {
            combine,
            points: vec![from],
            reach: Some(from),
            polygon: true,
        }
    }

    pub(crate) fn is_polygon(&self) -> bool {
        self.polygon
    }

    pub(crate) fn extend(&mut self, to: Point, spacing: f32) {
        let far = self
            .points
            .last()
            .is_none_or(|last| (to.x - last.x).hypot(to.y - last.y) >= spacing);
        if far {
            self.points.push(to);
        }
    }

    pub(crate) fn aim(&mut self, at: Point) -> bool {
        let moved = self.reach != Some(at);
        self.reach = Some(at);
        moved
    }

    pub(crate) fn closes(&self, at: Point, presses: u32, scale: f32) -> bool {
        let doubled = presses >= 2;
        let first = self.points[0];
        let home =
            self.points.len() > 2 && (at.x - first.x).hypot(at.y - first.y) * scale <= CLOSE_SLOP;
        doubled || home
    }

    pub(crate) fn add(&mut self, at: Point) {
        self.points.push(at);
        self.reach = Some(at);
    }

    pub(crate) fn retract(&mut self) -> bool {
        self.points.pop();
        !self.points.is_empty()
    }

    pub(crate) fn shape(&self, smoothing: bool) -> Option<Shape> {
        let points: Vec<(f32, f32)> = self.points.iter().map(|point| (point.x, point.y)).collect();
        if points.len() < 3 {
            return None;
        }
        let shape = match self.polygon {
            true => Shape::polygon([points], FillRule::NonZero),
            false => Shape::lasso(points),
        };
        Some(shape.anti_aliased(smoothing))
    }

    pub(crate) fn draw(&self, ctx: &mut PaintCtx<'_>, place: &impl Fn(f32, f32) -> Point) {
        let placed: Vec<Point> = self
            .points
            .iter()
            .chain(self.reach.iter())
            .map(|point| place(point.x, point.y))
            .collect();
        paint_dashed(ctx, &placed, false, &[(DASH, DARK), (DASH, LIGHT)]);
        if self.polygon {
            for point in &placed[..self.points.len()] {
                ctx.draw_circle(*point, VERTEX, LIGHT);
                ctx.draw_circle(*point, VERTEX_CORE, DARK);
            }
        }
    }
}
