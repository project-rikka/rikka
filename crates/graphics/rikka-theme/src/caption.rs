use crate::typography::Typography;

pub const HEIGHT: f32 = 38.0;

pub const SEPARATOR: f32 = 1.0;

pub const BAR: f32 = HEIGHT + SEPARATOR;

pub const BUTTON_WIDTH: f32 = 46.0;

pub const GLYPH: f32 = 16.0;

pub const TITLE_INSET: f32 = 12.0;

pub const RESIZE_BORDER: f32 = 5.0;

pub const RESIZE_CORNER: f32 = 16.0;

const TITLE_SCALE: f32 = 0.87;

#[must_use]
pub fn title_size(typography: &Typography) -> f32 {
    (typography.body_size * TITLE_SCALE).round()
}
