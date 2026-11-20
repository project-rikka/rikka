use std::ops::Range;

use rikka_theme::Palette;
use rikka_types::Color;

const THICKNESS: f32 = 1.0;
const CONVERTING_THICKNESS: f32 = 2.0;

pub(crate) fn rules(
    palette: &Palette,
    span: Range<usize>,
    converting: Option<Range<usize>>,
) -> [(Range<usize>, f32, Color); 2] {
    [
        (span, THICKNESS, palette.text_muted),
        (
            converting.unwrap_or(0..0),
            CONVERTING_THICKNESS,
            palette.accent,
        ),
    ]
}
