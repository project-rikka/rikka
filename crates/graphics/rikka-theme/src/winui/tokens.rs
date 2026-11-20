use rikka_types::Color;

use crate::variant::Variant;

pub(super) struct Tokens {
    pub(super) solid_background_fill_base: Color,
    pub(super) card_background_fill_default: Color,
    pub(super) control_alt_fill_quarternary: Color,
    pub(super) text_fill_primary: Color,
    pub(super) text_fill_secondary: Color,
    pub(super) text_fill_tertiary: Color,
    pub(super) text_on_accent_fill_primary: Color,
    pub(super) surface_stroke_default: Color,
    pub(super) system_fill_critical: Color,
    pub(super) system_fill_caution: Color,
    pub(super) system_fill_success: Color,
}

const LIGHT: Tokens = Tokens {
    solid_background_fill_base: argb(0xFFF3F3F3),
    card_background_fill_default: argb(0xB3FFFFFF),
    control_alt_fill_quarternary: argb(0x18000000),
    text_fill_primary: argb(0xE4000000),
    text_fill_secondary: argb(0x9E000000),
    text_fill_tertiary: argb(0x72000000),
    text_on_accent_fill_primary: argb(0xFFFFFFFF),
    surface_stroke_default: argb(0x66757575),
    system_fill_critical: argb(0xFFC42B1C),
    system_fill_caution: argb(0xFF9D5D00),
    system_fill_success: argb(0xFF0F7B0F),
};

const DARK: Tokens = Tokens {
    solid_background_fill_base: argb(0xFF202020),
    card_background_fill_default: argb(0x0DFFFFFF),
    control_alt_fill_quarternary: argb(0x12FFFFFF),
    text_fill_primary: argb(0xFFFFFFFF),
    text_fill_secondary: argb(0xC5FFFFFF),
    text_fill_tertiary: argb(0x87FFFFFF),
    text_on_accent_fill_primary: argb(0xFF000000),
    surface_stroke_default: argb(0x66757575),
    system_fill_critical: argb(0xFFFF99A4),
    system_fill_caution: argb(0xFFFCE100),
    system_fill_success: argb(0xFF6CCB5F),
};

impl Tokens {
    pub(super) const fn of(variant: Variant) -> &'static Tokens {
        match variant {
            Variant::Light => &LIGHT,
            Variant::Dark => &DARK,
        }
    }
}

pub(super) const fn argb(value: u32) -> Color {
    Color::rgba(
        channel(value, 16),
        channel(value, 8),
        channel(value, 0),
        channel(value, 24),
    )
}

const fn channel(value: u32, shift: u32) -> f32 {
    ((value >> shift) & 0xff) as f32 / 255.0
}
