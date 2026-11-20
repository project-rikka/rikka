use crate::prelude::*;
use rikka_widgets::column;

const GLYPH: f32 = 34.0;
const WIDEST: f32 = 460.0;

pub fn empty_state(content: impl Into<Label>) -> impl View {
    container(
        column! {
            icon(MaterialSymbol::FORUM)
                .extent(Length::px(GLYPH))
                .color(palette().text_muted);
            text(content)
                .label()
                .align(TextAlign::Center)
                .muted()
                .max_width(WIDEST);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center),
    )
    .center()
    .grow(1.0)
    .fill_width()
}
