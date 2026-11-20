use rikka_theme::{Theme, caption};
use rikka_types::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CaptionLook {
    pub(crate) fill: Color,
    pub(crate) separator: Color,
    pub(crate) title: Color,
    pub(crate) title_unfocused: Color,
    pub(crate) hover: Color,
    pub(crate) press: Color,
    pub(crate) danger: Color,
    pub(crate) danger_pressed: Color,
    pub(crate) on_danger: Color,
    pub(crate) title_size: f32,
}

impl CaptionLook {
    pub(crate) fn of(theme: &Theme) -> Self {
        let palette = theme.palette();
        Self {
            fill: over(palette.background, palette.background),
            separator: palette.outline,
            title: palette.text,
            title_unfocused: palette.text_muted,
            hover: theme.hover_layer(),
            press: theme.press_layer(),
            danger: palette.danger,
            danger_pressed: theme.pressed(palette.danger),
            on_danger: palette.on_danger,
            title_size: caption::title_size(theme.typography()),
        }
    }
}

fn over(top: Color, bottom: Color) -> Color {
    let alpha = top.a + bottom.a * (1.0 - top.a);
    if alpha <= 0.0 {
        return Color::TRANSPARENT;
    }
    let mix = |above: f32, below: f32| (above * top.a + below * bottom.a * (1.0 - top.a)) / alpha;
    Color::rgba(
        mix(top.r, bottom.r),
        mix(top.g, bottom.g),
        mix(top.b, bottom.b),
        alpha,
    )
}
