use rikka_types::{Color, Point, Rect};
use rikka_ui::{Element, Layout, PaintCtx, Quad, Style, Styled, View, Widget};

use crate::widgets::paint::track::track;

const MARK_RING: f32 = 2.0;

#[derive(Debug)]
pub(crate) struct Span {
    from: f32,
    to: f32,
    mark: Option<f32>,
    color: Color,
    style: Style,
}

#[must_use]
pub(crate) fn span(from: f32, to: f32, color: Color) -> Span {
    let (from, to) = (from.clamp(0.0, 1.0), to.clamp(0.0, 1.0));
    Span {
        from: from.min(to),
        to: from.max(to),
        mark: None,
        color,
        style: Style::default(),
    }
}

impl Span {
    #[must_use]
    pub(crate) fn marked(mut self, at: f32) -> Span {
        self.mark = Some(at.clamp(0.0, 1.0));
        self
    }
}

impl Widget for Span {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let round = bounds.height() / 2.0;
        let track = track(ctx);
        ctx.draw_quad(Quad::new(bounds).fill(track).radius(round));
        let start = bounds.x() + bounds.width() * self.from;
        let width = (bounds.width() * (self.to - self.from)).max(bounds.height());
        let start = start.min(bounds.max_x() - width);
        ctx.draw_quad(
            Quad::new(Rect::new(start, bounds.y(), width, bounds.height()))
                .fill(self.color)
                .radius(round),
        );
        if let Some(mark) = self.mark {
            let reach = bounds.height() + MARK_RING * 2.0;
            let center = Point::new(
                (bounds.x() + bounds.width() * mark)
                    .clamp(bounds.x() + reach / 2.0, bounds.max_x() - reach / 2.0),
                bounds.center().y,
            );
            ctx.draw_circle(center, reach, ctx.palette().surface);
            ctx.draw_circle(center, bounds.height(), ctx.env().text_color);
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.from == other.from
            && self.to == other.to
            && self.mark == other.mark
            && self.color == other.color
            && self.style == other.style
    }
}

impl Styled for Span {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl View for Span {
    fn build(self) -> Element {
        Element::new(self)
    }
}
