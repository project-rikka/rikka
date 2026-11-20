use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_protocols::zwlr_layer_shell_v1::Layer as Level;
use rikka_compositor_protocols::zwlr_layer_surface_v1::Anchor;
use rikka_compositor_shell::{LayerId, LayerState, Margins};

const LEVELS_FROM_TOP: [Level; 4] = [Level::Overlay, Level::Top, Level::Bottom, Level::Background];

#[derive(Debug)]
pub(crate) struct Arranged {
    pub(crate) placed: Vec<(LayerId, Option<Rect<i32, Logical>>)>,
    pub(crate) work_area: Rect<i32, Logical>,
}

pub(crate) fn arrange(full: Rect<i32, Logical>, layers: &[(LayerId, LayerState)]) -> Arranged {
    let mut usable = Area::from(full);
    let mut placed = Vec::with_capacity(layers.len());
    for exclusive in [true, false] {
        for level in LEVELS_FROM_TOP {
            for (layer, state) in layers {
                if state.layer != level || (state.exclusive_zone > 0) != exclusive {
                    continue;
                }
                let bounds = if state.exclusive_zone < 0 {
                    Area::from(full)
                } else {
                    usable
                };
                let fit = fit(bounds, state);
                if fit.is_some() {
                    reserve(&mut usable, state);
                }
                placed.push((*layer, fit));
            }
        }
    }
    Arranged {
        placed,
        work_area: usable.into(),
    }
}

#[derive(Debug, Clone, Copy)]
struct Area {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl From<Rect<i32, Logical>> for Area {
    fn from(rect: Rect<i32, Logical>) -> Self {
        Self {
            x: rect.loc.x,
            y: rect.loc.y,
            w: rect.size.w,
            h: rect.size.h,
        }
    }
}

impl From<Area> for Rect<i32, Logical> {
    fn from(area: Area) -> Self {
        Rect::new(
            Point::new(area.x, area.y),
            Size::new(area.w.max(0), area.h.max(0)),
        )
    }
}

fn fit(bounds: Area, state: &LayerState) -> Option<Rect<i32, Logical>> {
    let anchor = state.anchor;
    let margins = state.margins;
    let (x, w) = span(
        Span {
            start: bounds.x,
            length: bounds.w,
            desired: state.size.w,
            margin_start: margins.left,
            margin_end: margins.right,
        },
        anchor.contains(Anchor::LEFT),
        anchor.contains(Anchor::RIGHT),
    );
    let (y, h) = span(
        Span {
            start: bounds.y,
            length: bounds.h,
            desired: state.size.h,
            margin_start: margins.top,
            margin_end: margins.bottom,
        },
        anchor.contains(Anchor::TOP),
        anchor.contains(Anchor::BOTTOM),
    );
    (w >= 0 && h >= 0).then(|| Rect::new(Point::new(x, y), Size::new(w, h)))
}

#[derive(Debug, Clone, Copy)]
struct Span {
    start: i32,
    length: i32,
    desired: i32,
    margin_start: i32,
    margin_end: i32,
}

fn span(span: Span, at_start: bool, at_end: bool) -> (i32, i32) {
    let end = span.start.saturating_add(span.length);
    if span.desired == 0 {
        return (
            span.start.saturating_add(span.margin_start),
            span.length
                .saturating_sub(span.margin_start)
                .saturating_sub(span.margin_end),
        );
    }
    let start = match (at_start, at_end) {
        (true, false) => span.start.saturating_add(span.margin_start),
        (false, true) => end
            .saturating_sub(span.desired)
            .saturating_sub(span.margin_end),
        _ => span
            .start
            .saturating_add(span.length / 2)
            .saturating_sub(span.desired / 2),
    };
    (start, span.desired)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

fn exclusive_edge(state: &LayerState) -> Option<Edge> {
    if state.exclusive_zone <= 0 {
        return None;
    }
    let chosen = if state.exclusive_edge.is_empty() {
        state.anchor
    } else {
        state.exclusive_edge
    };
    let across = Anchor::LEFT | Anchor::RIGHT;
    let along = Anchor::TOP | Anchor::BOTTOM;
    [
        (Anchor::TOP, across, Edge::Top),
        (Anchor::BOTTOM, across, Edge::Bottom),
        (Anchor::LEFT, along, Edge::Left),
        (Anchor::RIGHT, along, Edge::Right),
    ]
    .into_iter()
    .find(|&(edge, perpendicular, _)| chosen == edge || chosen == edge | perpendicular)
    .map(|(_, _, edge)| edge)
}

fn reserve(usable: &mut Area, state: &LayerState) {
    let Some(edge) = exclusive_edge(state) else {
        return;
    };
    let Margins {
        top,
        right,
        bottom,
        left,
    } = state.margins;
    let margin = match edge {
        Edge::Top => top,
        Edge::Bottom => bottom,
        Edge::Left => left,
        Edge::Right => right,
    };
    let taken = state.exclusive_zone.saturating_add(margin);
    if taken <= 0 {
        return;
    }
    match edge {
        Edge::Top => {
            usable.y = usable.y.saturating_add(taken);
            usable.h = usable.h.saturating_sub(taken);
        }
        Edge::Bottom => usable.h = usable.h.saturating_sub(taken),
        Edge::Left => {
            usable.x = usable.x.saturating_add(taken);
            usable.w = usable.w.saturating_sub(taken);
        }
        Edge::Right => usable.w = usable.w.saturating_sub(taken),
    }
}
