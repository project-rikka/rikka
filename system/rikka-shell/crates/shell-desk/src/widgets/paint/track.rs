use rikka_types::Color;
use rikka_ui::PaintCtx;

const TRACK_ALPHA: f32 = 0.14;

#[must_use]
pub(crate) fn track(ctx: &PaintCtx<'_>) -> Color {
    ctx.env().text_color.scale_alpha(TRACK_ALPHA)
}
