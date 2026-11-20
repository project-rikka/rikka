use cut_project::MarkerColor;
use rikka_app::prelude::*;

pub(crate) const FLAG: f32 = 8.0;
const RANGE: f32 = 3.0;
const STEM: f32 = 1.0;

pub(crate) fn color(color: MarkerColor) -> Color {
    match color {
        MarkerColor::Green => Color::rgb(0.30, 0.74, 0.35),
        MarkerColor::Red => Color::rgb(0.91, 0.30, 0.24),
        MarkerColor::Purple => Color::rgb(0.66, 0.40, 0.85),
        MarkerColor::Orange => Color::rgb(0.95, 0.58, 0.18),
        MarkerColor::Yellow => Color::rgb(0.96, 0.82, 0.22),
        MarkerColor::White => Color::rgb(0.93, 0.94, 0.95),
        MarkerColor::Blue => Color::rgb(0.25, 0.55, 0.92),
        MarkerColor::Cyan => Color::rgb(0.18, 0.78, 0.80),
    }
}

pub(crate) fn paint_flag(ctx: &mut PaintCtx<'_>, x: f32, top: f32, bottom: f32, ink: Color) {
    ctx.draw_quad(
        Quad::new(Rect::new(x - FLAG / 2.0, top, FLAG, FLAG))
            .fill(ink)
            .radius(Corner::Detail),
    );
    if bottom > top + FLAG {
        ctx.draw_quad(
            Quad::new(Rect::new(
                x - STEM / 2.0,
                top + FLAG,
                STEM,
                bottom - top - FLAG,
            ))
            .fill(ink),
        );
    }
}

pub(crate) fn paint_range(ctx: &mut PaintCtx<'_>, from: f32, to: f32, top: f32, ink: Color) {
    if to <= from {
        return;
    }
    ctx.draw_quad(Quad::new(Rect::new(from, top, to - from, RANGE)).fill(ink));
}
