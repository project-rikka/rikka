use rikka_app::prelude::*;

use browser_model::SpaceIcon;

pub(crate) const RAIL: f32 = 52.0;
pub(crate) const PANEL: f32 = 280.0;
pub(crate) const ROW: f32 = 32.0;
pub(crate) const GAP: f32 = 2.0;
pub(crate) const PITCH: f32 = ROW + GAP;
pub(crate) const INDENT: f32 = 14.0;
pub(crate) const FAVICON: f32 = 16.0;
pub(crate) const EDGE: f32 = 8.0;

pub(crate) fn space_mark(mark: &SpaceIcon, side: f32, color: Color) -> Element {
    match mark {
        SpaceIcon::Symbol(symbol) => icon(symbol.glyph())
            .extent(Length::px(side))
            .color(color)
            .any(),
        SpaceIcon::Emoji(emoji) => text(emoji.clone()).text_size(side * 0.9).no_wrap().any(),
    }
}

pub(crate) fn folder_symbol(open: bool) -> MaterialSymbol {
    match open {
        true => MaterialSymbol::FOLDER_OPEN,
        false => MaterialSymbol::FOLDER,
    }
}

pub(crate) fn space_tint(chosen: bool) -> Color {
    match chosen {
        true => palette().text,
        false => palette().text_muted,
    }
}

pub(crate) fn rule() -> Element {
    container(
        container(spacer())
            .background(palette().outline.with_alpha(0.6))
            .height(1.0)
            .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::TIGHT))
    .fill_width()
    .any()
}
