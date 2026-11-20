use rikka_app::prelude::*;

pub(crate) fn glyph_button(symbol: MaterialSymbol, side: f32) -> Button {
    button(icon(symbol).extent(Length::px(side)))
        .ghost()
        .icon_only()
        .small()
        .focusless()
}
