use rikka_symbols::MaterialSymbol;
use rikka_types::Color;
use rikka_ui::{Corner, Length, palette};

use crate::blocks::container::{Container, container};
use crate::media::icon::icon;

const ACTIVE_TINT: f32 = 0.16;

pub fn tool_tile(symbol: MaterialSymbol, glyph: f32, active: bool) -> Container {
    let accent = palette().accent;
    let (fill, ink) = match active {
        true => (accent.scale_alpha(ACTIVE_TINT), accent),
        false => (Color::TRANSPARENT, palette().text),
    };
    container(icon(symbol).extent(Length::px(glyph)).color(ink))
        .background(fill)
        .radius(Corner::Item)
        .center()
        .focusless()
}
