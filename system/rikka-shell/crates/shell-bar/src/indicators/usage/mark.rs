use rikka_ui::{Align, Justify, Length, MaterialSymbol, Styled, View, palette};
use rikka_widgets::{icon, row};

const SIDE: f32 = 19.0;
const SYMBOL: f32 = 17.0;

#[must_use]
pub(crate) fn symbol() -> impl View {
    row! {
        icon(MaterialSymbol::AUTO_AWESOME)
            .extent(Length::px(SYMBOL))
            .color(palette().text);
    }
    .justify(Justify::Center)
    .align_items(Align::Center)
    .size(Length::px(SIDE), Length::px(SIDE))
}
