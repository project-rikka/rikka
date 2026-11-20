use rikka_types::Color;

use crate::ansi::Ink;
use crate::hues;
use crate::palette::Palette;
use crate::theme::{Theme, overlay};
use crate::tone;
use crate::translucency::Translucency;
use crate::variant::Variant;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SystemColors {
    pub window: Color,
    pub window_text: Color,
    pub highlight: Color,
    pub highlight_text: Color,
    pub hotlight: Color,
    pub gray_text: Color,
    pub button_face: Color,
    pub button_text: Color,
}

impl SystemColors {
    pub fn variant(&self) -> Variant {
        match tone::measure(self.window) < tone::measure(self.window_text) {
            true => Variant::Dark,
            false => Variant::Light,
        }
    }
}

pub fn high_contrast(colors: &SystemColors) -> Theme {
    let variant = colors.variant();
    let opaque_palette = Palette {
        background: colors.window,
        surface: colors.window,
        surface_variant: colors.button_face,
        text: colors.window_text,
        text_muted: colors.window_text,
        accent: colors.highlight,
        on_accent: colors.highlight_text,
        selection: colors.highlight,
        on_selection: colors.highlight_text,
        danger: colors.window_text,
        on_danger: colors.window,
        warning: colors.window_text,
        on_warning: colors.window,
        success: colors.window_text,
        on_success: colors.window,
        outline: colors.button_text,
        overlay: overlay(variant),
    };
    let ink = Ink {
        background: colors.window,
        text: colors.window_text,
        subtext: colors.window_text,
        muted: colors.gray_text,
        cursor: colors.window_text,
    };
    Theme::assemble(
        opaque_palette,
        Translucency::opaque(),
        &hues::harmonized(colors.hotlight, variant),
        &ink,
        variant,
    )
}
