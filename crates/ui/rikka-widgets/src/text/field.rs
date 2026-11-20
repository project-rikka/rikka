use rikka_types::Rect;
use rikka_ui::{PaintCtx, Quad, motion};

const BORDER: f32 = 1.0;
const ATTENTIVE_WEIGHT: f32 = 2.0;

pub(crate) fn text_top(ctx: &PaintCtx<'_>, bounds: Rect, line_height: f32) -> f32 {
    let centred = bounds.y() + (bounds.height() - line_height) / 2.0;
    ctx.snap(centred) - bounds.y()
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FieldMood {
    pub(crate) enabled: bool,
    pub(crate) attentive: bool,
    pub(crate) hovered: bool,
    pub(crate) invalid: bool,
}

pub(crate) fn paint_frame(ctx: &mut PaintCtx<'_>, bounds: Rect, radius: f32, mood: FieldMood) {
    let palette = ctx.theme().palette();
    let border = match () {
        _ if !mood.enabled => ctx.theme().disabled(palette.outline),
        _ if mood.invalid => palette.danger,
        _ if mood.attentive => palette.accent,
        _ if mood.hovered => palette.text_muted,
        _ => palette.outline,
    };
    let background = match mood.enabled {
        true => palette.surface_variant,
        false => palette.surface,
    };
    let border = ctx.animate_curve("frame.border", border, motion::COLOR);
    let weight = ctx.animate_curve(
        "frame.weight",
        if mood.attentive {
            BORDER * ATTENTIVE_WEIGHT
        } else {
            BORDER
        },
        motion::MORPH,
    );
    let background = ctx.animate_curve("frame.background", background, motion::COLOR);
    ctx.draw_quad(
        Quad::new(bounds)
            .fill(background)
            .stroke(weight, border)
            .radius(radius),
    );
}
