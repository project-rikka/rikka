use std::hash::Hash;

use rikka_types::{Color, Point, Rect, Size};
use rikka_ui::motion::indicator::{self, Indicator};
use rikka_ui::{Axis, Corner, PaintCtx, Quad, motion};

const HOVER: (&str, &str) = ("highlights", "hover");

const SELECTION: (&str, &str) = ("highlights", "selection");

#[derive(Debug, Default)]
struct Fill {
    indicator: Indicator,
    shape: Option<(f32, Size)>,
}

impl Fill {
    fn resolve(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        key: impl Hash + Copy,
        along: Axis,
        target: Option<(usize, Rect)>,
        color: Color,
    ) -> Option<(Rect, Color)> {
        if let Some((_, rect)) = target {
            self.shape = Some((cross_coord(along, rect), rect.size));
        }
        let slot = target.map(|(cell, rect)| (cell, along.coord(rect.origin)));
        let (position, tint) = self
            .indicator
            .resolve(ctx, key, slot, color, motion::INDICATOR)?;
        let (cross, size) = self.shape?;
        let origin = along.point(position, cross);
        Some((Rect::from_origin_size(origin, size), tint))
    }
}

fn cross_coord(along: Axis, rect: Rect) -> f32 {
    along.across().coord(rect.origin)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct HighlightRect {
    along: Axis,
    fill: Option<Rect>,
}

impl HighlightRect {
    pub fn emphasis(self, cell: Rect) -> f32 {
        match self.fill {
            Some(fill) => indicator::emphasis(
                self.along.coord(fill.origin),
                self.along.coord(cell.origin),
                self.along.length(fill.size),
            ),
            None => 0.0,
        }
    }
}

#[derive(Debug, Default)]
pub struct Highlights {
    hover: Fill,
    selection: Fill,
}

impl Highlights {
    pub fn paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        along: Axis,
        cell_rect: impl Fn(usize) -> Rect,
        selected: Option<usize>,
        hovered: Option<usize>,
    ) -> HighlightRect {
        let radius = Corner::Item.radius();
        let (hover_color, selection_color) = {
            let theme = ctx.theme();
            (theme.hover_layer(), theme.palette().selection)
        };

        let origin = ctx.bounds().origin;
        let local_rect = |cell| cell_rect(cell).translate(Point::ZERO - origin);
        let hover_target = hovered
            .filter(|cell| selected != Some(*cell))
            .map(|cell| (cell, local_rect(cell)));
        if let Some((rect, tint)) = self
            .hover
            .resolve(ctx, HOVER, along, hover_target, hover_color)
        {
            ctx.draw_quad(Quad::new(rect.translate(origin)).fill(tint).radius(radius));
        }

        let selection_target = selected.map(|cell| (cell, local_rect(cell)));
        let Some((rect, tint)) =
            self.selection
                .resolve(ctx, SELECTION, along, selection_target, selection_color)
        else {
            return HighlightRect { along, fill: None };
        };
        let rect = rect.translate(origin);
        ctx.draw_quad(Quad::new(rect).fill(tint).radius(radius));
        HighlightRect {
            along,
            fill: Some(rect),
        }
    }

    pub(crate) fn clear(&mut self, ctx: &mut PaintCtx<'_>) {
        self.hover.indicator.vanish(ctx, HOVER);
        self.selection.indicator.vanish(ctx, SELECTION);
    }
}

pub(crate) fn paint_selection(ctx: &mut PaintCtx<'_>, rect: Rect, clip: Rect) {
    let fill = ctx.theme().palette().selection;
    let quad = Quad::new(rect).fill(fill).radius(Corner::Item.radius());
    ctx.clipped(clip, |ctx| ctx.draw_quad(quad));
}
