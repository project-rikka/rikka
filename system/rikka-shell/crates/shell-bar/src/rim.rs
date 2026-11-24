use rikka_types::{CornerRadii, Rect, RoundedRect};
use rikka_ui::{DurationToken, PaintCtx, Quad, Transition, motion, offset, theme};
use shell_kit::taskbar::{BORDER, CORNER, Edge};

pub(crate) const EXIT_DURATION: DurationToken = DurationToken::Short4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Corners {
    Rounded,
    Square,
}

pub(crate) fn paint(ctx: &mut PaintCtx<'_>, edge: Edge, depth: f32, corners: Corners) {
    let bounds = ctx.bounds();
    if bounds.is_empty() {
        return;
    }
    let fill = theme().palette().background;
    let length = edge.length(bounds);
    let band = edge.place(bounds, Rect::new(0.0, 0.0, length, depth));
    ctx.draw_quad(Quad::new(band).fill(fill));
    ctx.glass(band);
    if corners == Corners::Square {
        return;
    }
    let inner = (length - BORDER * 2.0).max(0.0);
    let hang = edge.place(bounds, Rect::new(BORDER, depth, inner, CORNER));
    let desktop = RoundedRect::new(
        edge.place(bounds, Rect::new(BORDER, depth, inner, CORNER * 2.0)),
        edge.radii(CornerRadii::new(CORNER, CORNER, 0.0, 0.0)),
    );
    ctx.draw_quad(Quad::new(hang).fill(fill).cut_out(desktop));
}

#[must_use]
pub(crate) fn docking(edge: Edge, travel: f32) -> Transition {
    shifted(edge, travel)
        .enter(motion::SLIDE_IN)
        .exit(motion::SLIDE_OUT)
}

#[must_use]
pub(crate) fn shifted(edge: Edge, distance: f32) -> Transition {
    let away = edge.outward(distance);
    offset(away.x, away.y)
}
