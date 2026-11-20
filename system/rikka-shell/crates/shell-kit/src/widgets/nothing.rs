use rikka_ui::{
    Align, Justify, Label, Length, MaterialSymbol, Space, TextStyled, View, palette, text,
};
use rikka_widgets::{icon, row};

const SYMBOL: f32 = 16.0;
const PADDING: [f32; 2] = [10.0, 20.0];

#[must_use]
pub fn nothing(symbol: MaterialSymbol, line: Label) -> impl View {
    row! {
        icon(symbol)
            .extent(Length::px(SYMBOL))
            .color(palette().text_muted);
        text(line).caption().muted()
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .justify(Justify::Center)
    .padding(PADDING)
}
