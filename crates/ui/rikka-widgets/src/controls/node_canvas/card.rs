use std::sync::Arc;

use rikka_types::{Color, Point, Rect};
use rikka_ui::{Behavior, Layout, Length, PaintCtx, Quad, TextRun, Widget, palette, text_width};

use crate::controls::node_canvas::geometry::{
    HEADER, MANY_SPREAD, PORT, ROW, node_height, port_offset,
};
use crate::controls::node_canvas::model::{CanvasNode, NodeGraph, PortSide};

const TITLE: f32 = 12.5;
const LABEL: f32 = 11.5;
const INSET: f32 = 10.0;
const RADIUS: f32 = 7.0;
const BADGE: f32 = 14.0;
const SWATCH: f32 = 12.0;
const DETAILED_ZOOM: f32 = 0.45;
const RING: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Lit {
    pub(super) selected: bool,
    pub(super) focused: bool,
    pub(super) hovered: bool,
    pub(super) hot: Option<(PortSide, usize)>,
}

pub(super) struct Card {
    pub(super) graph: Arc<NodeGraph>,
    pub(super) at: usize,
    pub(super) zoom: f32,
    pub(super) lit: Lit,
    pub(super) wired_inputs: Vec<bool>,
    pub(super) wired_outputs: Vec<bool>,
}

impl Card {
    fn node(&self) -> &CanvasNode {
        &self.graph.nodes[self.at]
    }
}

pub(super) fn badge_area(node: &CanvasNode, zoom: f32) -> Rect {
    Rect::new(
        (node.width - INSET - BADGE) * zoom,
        (HEADER - BADGE) * 0.5 * zoom,
        BADGE * zoom,
        BADGE * zoom,
    )
}

fn header_color(accent: Color, surface: Color) -> Color {
    accent.mix(surface, 0.35)
}

fn ink_on(color: Color) -> Color {
    let light = color.r * 0.2126 + color.g * 0.7152 + color.b * 0.0722;
    match light > 0.55 {
        true => Color::rgb(0.08, 0.08, 0.1),
        false => Color::rgb(0.96, 0.96, 0.98),
    }
}

impl Widget for Card {
    type State = ();

    fn layout(&self) -> Layout {
        let node = self.node();
        Layout::column()
            .size(
                Length::px(node.width * self.zoom),
                Length::px(node_height(node) * self.zoom),
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
        let node = self.node();
        let zoom = self.zoom;
        let bounds = ctx.bounds();
        let origin = bounds.origin;
        let radius = RADIUS * zoom;
        let border = match (self.lit.selected, self.lit.hovered) {
            (true, _) => palette.accent,
            (false, true) => palette.text_muted,
            (false, false) => palette.outline,
        };
        if self.lit.focused {
            ctx.draw_quad(
                Quad::new(bounds.inflate(RING * 2.0))
                    .stroke(RING, palette.accent.scale_alpha(0.55))
                    .radius(radius + RING * 2.0),
            );
        }
        ctx.draw_quad(
            Quad::new(bounds)
                .fill(palette.surface_variant)
                .stroke(if self.lit.selected { 2.0 } else { 1.0 }, border)
                .radius(radius),
        );
        let header = Rect::new(origin.x, origin.y, bounds.width(), HEADER * zoom);
        let tint = header_color(node.accent, palette.surface_variant);
        ctx.draw_quad(
            Quad::new(header.inflate(-1.0))
                .fill(tint)
                .radius(rikka_types::CornerRadii::new(radius, radius, 0.0, 0.0)),
        );
        let detailed = zoom >= DETAILED_ZOOM;
        if detailed {
            let room = match node.problem {
                Some(_) => node.width - INSET * 2.0 - BADGE - 4.0,
                None => node.width - INSET * 2.0,
            };
            ctx.draw_text(
                Rect::new(
                    origin.x + INSET * zoom,
                    origin.y + (HEADER - TITLE * 1.4) * 0.5 * zoom,
                    room * zoom,
                    TITLE * 1.4 * zoom,
                ),
                TextRun::new(node.title.clone(), TITLE * zoom)
                    .medium()
                    .no_wrap()
                    .ellipsized(),
                ink_on(tint),
            );
        }
        if node.problem.is_some() {
            let badge = badge_area(node, zoom).translate(origin);
            ctx.draw_circle(badge.center(), badge.width(), palette.danger);
            if detailed {
                let run = TextRun::new("!", LABEL * zoom).medium().no_wrap();
                let width = text_width(&run);
                ctx.draw_text(
                    Rect::new(
                        badge.center().x - width * 0.5,
                        badge.y(),
                        width + 1.0,
                        badge.height(),
                    ),
                    run,
                    palette.on_danger,
                );
            }
        }
        if node.detail.is_some() || node.swatch.is_some() {
            let top = origin.y + HEADER * zoom;
            let mut x = origin.x + INSET * zoom;
            if let Some(swatch) = node.swatch {
                let side = SWATCH * zoom;
                ctx.draw_quad(
                    Quad::new(Rect::new(x, top + (ROW * zoom - side) * 0.5, side, side))
                        .fill(swatch.with_alpha(1.0))
                        .stroke(1.0, palette.outline)
                        .radius(3.0 * zoom),
                );
                x += side + 6.0 * zoom;
            }
            if detailed && let Some(detail) = &node.detail {
                ctx.draw_text(
                    Rect::new(
                        x,
                        top + (ROW - LABEL * 1.4) * 0.5 * zoom,
                        origin.x + bounds.width() - INSET * zoom - x,
                        LABEL * 1.4 * zoom,
                    ),
                    TextRun::new(detail.clone(), LABEL * zoom)
                        .no_wrap()
                        .ellipsized(),
                    palette.text_muted,
                );
            }
        }
        for (side, ports, wired) in [
            (PortSide::Output, &node.outputs, &self.wired_outputs),
            (PortSide::Input, &node.inputs, &self.wired_inputs),
        ] {
            for (index, port) in ports.iter().enumerate() {
                let anchor = origin + port_offset(node, side, index) * zoom;
                let hot = self.lit.hot == Some((side, index));
                let diameter = PORT * zoom * if hot { 1.35 } else { 1.0 };
                let connected = wired.get(index).copied().unwrap_or(false);
                let height = match port.many {
                    true => MANY_SPREAD * zoom + diameter,
                    false => diameter,
                };
                let socket = Rect::new(
                    anchor.x - diameter * 0.5,
                    anchor.y - height * 0.5,
                    diameter,
                    height,
                );
                ctx.draw_quad(
                    Quad::new(socket.inflate(1.0))
                        .fill(palette.surface_variant)
                        .radius(diameter * 0.5 + 1.0),
                );
                let shape = Quad::new(socket).radius(diameter * 0.5);
                ctx.draw_quad(match connected || hot {
                    true => shape.fill(port.color),
                    false => shape
                        .fill(palette.surface_variant)
                        .stroke((1.5 * zoom).max(1.0), port.color),
                });
                if !detailed {
                    continue;
                }
                let run = TextRun::new(port.label.clone(), LABEL * zoom).no_wrap();
                let line = LABEL * 1.4 * zoom;
                let room = node.width * zoom * 0.5 + INSET * zoom * 2.0;
                let width = text_width(&run).min(node.width * zoom - INSET * zoom * 2.0);
                let x = match side {
                    PortSide::Input => anchor.x + INSET * zoom,
                    PortSide::Output => anchor.x - INSET * zoom - width,
                };
                ctx.draw_text(
                    Rect::new(x, anchor.y - line * 0.5, width.min(room * 2.0) + 1.0, line),
                    run.ellipsized(),
                    palette.text,
                );
            }
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.zoom == other.zoom
            && self.lit == other.lit
            && self.wired_inputs == other.wired_inputs
            && self.wired_outputs == other.wired_outputs
            && (Arc::ptr_eq(&self.graph, &other.graph) && self.at == other.at
                || alike(self.node(), other.node()))
    }

    fn debug_name(&self) -> &'static str {
        "NodeCard"
    }
}
fn alike(one: &CanvasNode, two: &CanvasNode) -> bool {
    one.key == two.key
        && one.title == two.title
        && one.width == two.width
        && one.accent == two.accent
        && one.inputs == two.inputs
        && one.outputs == two.outputs
        && one.detail == two.detail
        && one.swatch == two.swatch
        && one.problem == two.problem
}

impl std::fmt::Debug for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeCard")
            .field("node", &self.node().key)
            .finish_non_exhaustive()
    }
}
