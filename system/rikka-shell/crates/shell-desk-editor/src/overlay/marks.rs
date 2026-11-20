use desk_layout::{Area, Layout, Projection, WidgetId};
use rikka_theme::{Corner, Theme};
use rikka_types::{Color, Point, Rect, Size};
use rikka_ui::{IconGlyph, PaintCtx, Quad, motion};
use shell_desk::widgets;

use super::toolbar;
use crate::session::{DragPreview, Hovered, Tool};

const RING: f32 = 2.0;
const HOVER_RING: f32 = 1.5;
const RING_GAP: f32 = 3.0;
const HOVER_ALPHA: f32 = 0.7;
const GHOST_ALPHA: f32 = 0.82;
const GHOST_SYMBOL: f32 = 32.0;
const GHOST_SHARE: f32 = 0.4;

pub(super) struct Shown<'a> {
    pub(super) layout: &'a Layout,
    pub(super) area: &'a Area,
    pub(super) projection: Projection,
    pub(super) grabbed: Option<WidgetId>,
    pub(super) theme: &'a Theme,
}

impl Shown<'_> {
    pub(super) fn hovered(&self, hovered: Hovered) -> Option<WidgetId> {
        match hovered {
            Hovered::Widget(id) if self.layout.get(id).is_some() => Some(id),
            _ => None,
        }
    }

    pub(super) fn hover_ring(&self, id: WidgetId, ctx: &mut PaintCtx<'_>) {
        let Some(desk) = self.layout.rect_of(id, self.area) else {
            return;
        };
        let color = self.theme.palette().text_muted.with_alpha(HOVER_ALPHA);
        ring(
            self.projection.rect(desk),
            self.projection.scale(),
            HOVER_RING,
            color,
            ctx,
        );
    }

    pub(super) fn selection(
        &self,
        id: WidgetId,
        card: Rect,
        hovered: Option<Hovered>,
        steady: bool,
        ctx: &mut PaintCtx<'_>,
    ) -> Vec<(Tool, Rect)> {
        let Some(widget) = self.layout.get(id) else {
            return Vec::new();
        };
        let key = ("ring", id.0);
        let desk = self.area.place(widget.size, widget.spot);
        if self.grabbed == Some(id) {
            ctx.jump(key, desk);
        }
        let desk = ctx.animate_curve(key, desk, motion::GLIDE);
        let rect = self.projection.rect(desk);
        let accent = self.theme.palette().accent;
        ring(rect, self.projection.scale(), RING, accent, ctx);
        if self.grabbed.is_some() || !steady {
            return Vec::new();
        }
        let hovered = match hovered {
            Some(Hovered::Tool(tool)) => Some(tool),
            _ => None,
        };
        let others: Vec<Rect> = self
            .layout
            .rects_except(id, self.area)
            .into_iter()
            .map(|desk| self.projection.rect(desk))
            .collect();
        let bar = toolbar::Bar {
            widget: id,
            sizes: widget.kind().sizes(),
            size: widget.size,
            hovered,
        };
        bar.draw(rect, card, &others, self.theme, ctx)
    }
}

fn ring(rect: Rect, scale: f32, width: f32, color: Color, ctx: &mut PaintCtx<'_>) {
    let radius = Corner::Window.radius() * scale + RING_GAP;
    ctx.draw_quad(
        Quad::new(rect.inflate(RING_GAP))
            .radius(radius)
            .stroke(width, color),
    );
}

pub(super) fn drag_preview(
    drag_preview: DragPreview,
    projection: Projection,
    theme: &Theme,
    ctx: &mut PaintCtx<'_>,
) {
    let palette = theme.palette();
    let scale = projection.scale();
    let extent = drag_preview.size.extent();
    let size = Size::new(extent.width * scale, extent.height * scale);
    let rect = Rect::from_origin_size(
        Point::new(
            drag_preview.at.x - drag_preview.grab.x * size.width,
            drag_preview.at.y - drag_preview.grab.y * size.height,
        ),
        size,
    );
    let radius = Corner::Window.radius() * scale;
    ctx.draw_quad(
        Quad::new(rect)
            .radius(radius)
            .fill(palette.surface.with_alpha(GHOST_ALPHA))
            .stroke(RING, palette.accent),
    );
    let symbol = (size.height * GHOST_SHARE).min(GHOST_SYMBOL);
    ctx.draw_icon(
        Rect::from_center(rect.center(), Size::new(symbol, symbol)),
        IconGlyph::from(widgets::about(drag_preview.kind).symbol).spec(symbol),
        palette.text,
    );
}
