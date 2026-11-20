use crate::prelude::*;

pub const CODE_FONT_SIZE: f32 = 12.0;

pub const LINE_HEIGHT_FACTOR: f32 = 1.5;

pub fn mono(content: impl Into<Label>, size: f32) -> Text {
    text(content)
        .family(Family::parse(font::JETBRAINS_MONO))
        .text_size(size)
        .line_height((size * LINE_HEIGHT_FACTOR).round())
}
