use desk_layout::Size;
use rikka_ui::{
    Align, Element, Justify, Label, Length, MaterialSymbol, Space, Styled, TextAlign, TextStyled,
    palette, text,
};
use rikka_widgets::{column, icon};

use crate::widgets::card::padded;

const SYMBOL_SMALL: f32 = 28.0;
const SYMBOL: f32 = 36.0;

pub(crate) struct Quiet {
    symbol: MaterialSymbol,
    title: Label,
    detail: Option<Label>,
}

#[must_use]
pub(crate) fn quiet(symbol: MaterialSymbol, title: impl Into<Label>) -> Quiet {
    Quiet {
        symbol,
        title: title.into(),
        detail: None,
    }
}

impl Quiet {
    #[must_use]
    pub(crate) fn detail(mut self, detail: impl Into<Label>) -> Quiet {
        self.detail = Some(detail.into());
        self
    }

    #[must_use]
    pub(crate) fn sized(self, size: Size) -> Element {
        let symbol = match size {
            Size::Small => SYMBOL_SMALL,
            Size::Medium | Size::Large | Size::ExtraLarge => SYMBOL,
        };
        let detail = self.detail.filter(|_| size != Size::Small);
        padded(
            column! {
                icon(self.symbol)
                    .extent(Length::px(symbol))
                    .color(palette().text_muted);
                text(self.title)
                    .label()
                    .medium()
                    .align(TextAlign::Center)
                    .fill_width();
                if let Some(detail) = detail {
                    text(detail)
                        .caption()
                        .muted()
                        .align(TextAlign::Center)
                        .fill_width();
                }
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .justify(Justify::Center)
            .fill(),
        )
    }
}
