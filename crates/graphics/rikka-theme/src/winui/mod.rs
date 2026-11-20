mod accent;
mod contrast;
mod tokens;

use rikka_types::Color;

use crate::ansi::Ink;
use crate::hues;
use crate::palette::Palette;
use crate::surfaces;
use crate::theme::{Theme, contrasting_text, overlay, preferred_text};
use crate::translucency::Translucency;
use crate::variant::Variant;
use crate::winui::tokens::Tokens;

pub use accent::{AccentPalette, from_colorref};
pub use contrast::{SystemColors, high_contrast};

pub fn theme(variant: Variant, accent: &AccentPalette) -> Theme {
    let tokens = Tokens::of(variant);
    let background = tokens.solid_background_fill_base;
    let flat = |token: Color| over(token, background);
    let text = flat(tokens.text_fill_primary);
    let muted = flat(tokens.text_fill_secondary);
    let (fill, accent_text) = match variant {
        Variant::Light => (accent.dark1, accent.dark2),
        Variant::Dark => (accent.light2, accent.light3),
    };
    let on_accent = preferred_text(
        fill,
        tokens.text_on_accent_fill_primary,
        Tokens::of(opposite(variant)).text_on_accent_fill_primary,
    );
    let selection = surfaces::selection(background, variant, fill);
    let foreground = |fill: Color| contrasting_text(fill, background, text);
    let opaque_palette = Palette {
        background,
        surface: flat(tokens.card_background_fill_default),
        surface_variant: flat(tokens.control_alt_fill_quarternary),
        text,
        text_muted: muted,
        accent: fill,
        on_accent,
        selection,
        on_selection: foreground(selection),
        danger: tokens.system_fill_critical,
        on_danger: foreground(tokens.system_fill_critical),
        warning: tokens.system_fill_caution,
        on_warning: foreground(tokens.system_fill_caution),
        success: tokens.system_fill_success,
        on_success: foreground(tokens.system_fill_success),
        outline: flat(tokens.surface_stroke_default),
        overlay: overlay(variant),
    };
    let ink = Ink {
        background,
        text,
        subtext: muted,
        muted: flat(tokens.text_fill_tertiary),
        cursor: accent_text,
    };
    Theme::assemble(
        opaque_palette,
        Translucency::opaque(),
        &hues::harmonized(accent.accent, variant),
        &ink,
        variant,
    )
}

fn opposite(variant: Variant) -> Variant {
    match variant {
        Variant::Light => Variant::Dark,
        Variant::Dark => Variant::Light,
    }
}

fn over(token: Color, ground: Color) -> Color {
    ground.mix(token.with_alpha(1.0), token.a)
}
