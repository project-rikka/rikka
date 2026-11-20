use rikka_ui::{Align, Inset, Length, MaterialSymbol, Space, Styled, View, palette};
use rikka_widgets::{TextField, icon, row};

const HEIGHT: f32 = 38.0;
const SYMBOL: f32 = 18.0;
const INSET: f32 = 10.0;

#[must_use]
pub fn finder(field: TextField) -> impl View {
    row! {
        icon(MaterialSymbol::SEARCH)
            .extent(Length::px(SYMBOL))
            .color(palette().text_muted);
        field.plain(true).autofocus(true).grow(1.0)
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(INSET, 0.0))
    .height(HEIGHT)
}
