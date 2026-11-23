use std::sync::Arc;

use rikka_types::{Color, Point, Rect};
use rikka_ui::{Behavior, Layout, Length, PaintCtx, Quad, TextRun, Widget, palette};

use crate::controls::node_canvas::geometry::{FRAME_TITLE, GRIP, Geometry, link_curve};
use crate::controls::node_canvas::model::{CanvasFrame, CanvasLink, FrameArea, NodeGraph};

const LINK: f32 = 2.5;
const LIT_LINK: f32 = 3.5;
const FRAME_TEXT: f32 = 12.5;

fn place(rect: Rect, zoom: f32) -> Rect {
    Rect::new(
        rect.x() * zoom,
        rect.y() * zoom,
        rect.width() * zoom,
        rect.height() * zoom,
    )
}

fn enclosing(points: impl IntoIterator<Item = Point>) -> Option<Rect> {
    let mut points = points.into_iter();
    let first = points.next()?;
    let (low, high) = points.fold((first, first), |(low, high), point| {
        (low.min(point), high.max(point))
    });
    Some(Rect::new(low.x, low.y, high.x - low.x, high.y - low.y))
}

pub(super) fn polyline(ctx: &mut PaintCtx<'_>, points: &[Point], width: f32, color: Color) {
    for pair in points.windows(2) {
        ctx.draw_segment(pair[0], pair[1], width, color);
    }
}

pub(super) struct Wires {
    pub(super) graph: Arc<NodeGraph>,
    pub(super) geometry: Arc<Geometry>,
    pub(super) zoom: f32,
    pub(super) area: Rect,
    pub(super) lit: Vec<CanvasLink>,
    pub(super) hidden: Option<CanvasLink>,
}

impl Wires {
    pub(super) fn area(geometry: &Geometry, zoom: f32) -> Rect {
        enclosing(
            geometry
                .link_points()
                .flat_map(|(_, points)| points.iter().copied()),
        )
        .map_or(Rect::ZERO, |area| place(area, zoom).inflate(LIT_LINK * 2.0))
    }
}

impl Widget for Wires {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column()
            .size(
                Length::px(self.area.width()),
                Length::px(self.area.height()),
            )
            .fixed()
    }

    fn behavior(&self, _state: &()) -> Behavior {
        Behavior::NONE
    }

    fn hit_test(&self, _bounds: Rect, _point: Point) -> bool {
        false
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let shift = ctx.bounds().origin - self.area.origin;
        let mut placed = Vec::new();
        for (link, points) in self.geometry.link_points() {
            if self.hidden == Some(link) {
                continue;
            }
            let lit = self.lit.contains(&link);
            let color = self
                .graph
                .nodes
                .iter()
                .find(|node| node.key == link.from.node)
                .and_then(|node| node.outputs.get(link.from.index))
                .map_or(palette.text_muted, |port| port.color);
            placed.clear();
            placed.extend(points.iter().map(|point| *point * self.zoom + shift));
            match lit {
                true => {
                    polyline(ctx, &placed, LIT_LINK * self.zoom.max(0.6), palette.accent);
                    polyline(ctx, &placed, LINK * self.zoom.max(0.6) * 0.5, color);
                }
                false => polyline(ctx, &placed, LINK * self.zoom.max(0.6), color),
            }
        }
    }

    fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.geometry, &other.geometry)
            && self.zoom == other.zoom
            && self.area == other.area
            && self.lit == other.lit
            && self.hidden == other.hidden
    }

    fn debug_name(&self) -> &'static str {
        "NodeWires"
    }
}

impl std::fmt::Debug for Wires {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeWires").finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Sketch {
    Link {
        from: Point,
        to: Point,
        color: Color,
    },
    Marquee(Rect),
}

impl Sketch {
    pub(super) fn area(&self, zoom: f32) -> Rect {
        match *self {
            Sketch::Link { from, to, .. } => enclosing(link_curve(from, to))
                .map_or(Rect::ZERO, |area| place(area, zoom).inflate(LIT_LINK * 2.0)),
            Sketch::Marquee(rect) => place(rect, zoom).inflate(2.0),
        }
    }
}

pub(super) struct Overlay {
    pub(super) sketch: Sketch,
    pub(super) zoom: f32,
}

impl Widget for Overlay {
    type State = ();

    fn layout(&self) -> Layout {
        let area = self.sketch.area(self.zoom);
        Layout::column()
            .size(Length::px(area.width()), Length::px(area.height()))
            .fixed()
    }

    fn behavior(&self, _state: &()) -> Behavior {
        Behavior::NONE
    }

    fn hit_test(&self, _bounds: Rect, _point: Point) -> bool {
        false
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let shift = ctx.bounds().origin - self.sketch.area(self.zoom).origin;
        match self.sketch {
            Sketch::Link { from, to, color } => {
                let points: Vec<Point> = link_curve(from, to)
                    .into_iter()
                    .map(|point| point * self.zoom + shift)
                    .collect();
                polyline(ctx, &points, LINK * self.zoom.max(0.6), color);
            }
            Sketch::Marquee(rect) => {
                let rect = place(rect, self.zoom).translate(shift);
                ctx.draw_quad(
                    Quad::new(rect)
                        .fill(palette.accent.scale_alpha(0.12))
                        .stroke(1.0, palette.accent.scale_alpha(0.8))
                        .radius(2.0),
                );
            }
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.sketch == other.sketch && self.zoom == other.zoom
    }

    fn debug_name(&self) -> &'static str {
        "NodeSketch"
    }
}

impl std::fmt::Debug for Overlay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeSketch")
            .field("sketch", &self.sketch)
            .finish()
    }
}

pub(super) struct Frame {
    pub(super) frame: CanvasFrame,
    pub(super) rect: Rect,
    pub(super) zoom: f32,
    pub(super) selected: bool,
}

impl Frame {
    pub(super) fn placed(&self) -> Rect {
        place(self.rect, self.zoom)
    }
}

impl Widget for Frame {
    type State = ();

    fn layout(&self) -> Layout {
        let placed = self.placed();
        Layout::column()
            .size(Length::px(placed.width()), Length::px(placed.height()))
            .fixed()
    }

    fn behavior(&self, _state: &()) -> Behavior {
        Behavior::NONE
    }

    fn hit_test(&self, _bounds: Rect, _point: Point) -> bool {
        false
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let bounds = ctx.bounds();
        let zoom = self.zoom;
        let color = self.frame.color;
        let border = match self.selected {
            true => palette.accent,
            false => color.scale_alpha(0.6),
        };
        ctx.draw_quad(
            Quad::new(bounds)
                .fill(color.scale_alpha(0.12))
                .stroke(if self.selected { 2.0 } else { 1.0 }, border)
                .radius(8.0 * zoom),
        );
        let comment = matches!(self.frame.area, FrameArea::At { .. });
        let title = Rect::new(
            bounds.x() + 10.0 * zoom,
            bounds.y() + 5.0 * zoom,
            (bounds.width() - 20.0 * zoom).max(0.0),
            match comment {
                true => (bounds.height() - 10.0 * zoom).max(0.0),
                false => (FRAME_TITLE - 6.0) * zoom,
            },
        );
        let run = TextRun::new(self.frame.title.clone(), FRAME_TEXT * zoom);
        let run = match comment {
            true => run,
            false => run.medium().no_wrap().ellipsized(),
        };
        ctx.draw_text(title, run, palette.text);
        if comment {
            let grip = GRIP * zoom.min(1.0);
            for step in 1..=3 {
                let reach = grip * step as f32 / 3.0;
                ctx.draw_segment(
                    Point::new(bounds.max_x() - reach, bounds.max_y() - 2.0),
                    Point::new(bounds.max_x() - 2.0, bounds.max_y() - reach),
                    1.0,
                    color.scale_alpha(0.8),
                );
            }
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.frame.title == other.frame.title
            && self.frame.color == other.frame.color
            && std::mem::discriminant(&self.frame.area) == std::mem::discriminant(&other.frame.area)
            && self.rect.size == other.rect.size
            && self.zoom == other.zoom
            && self.selected == other.selected
    }

    fn debug_name(&self) -> &'static str {
        "NodeFrame"
    }
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeFrame")
            .field("key", &self.frame.key)
            .finish_non_exhaustive()
    }
}
