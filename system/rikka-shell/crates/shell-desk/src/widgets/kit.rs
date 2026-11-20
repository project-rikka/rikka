use rikka_types::Color;
use rikka_ui::{
    Align, Element, Family, Label, Length, MaterialSymbol, Space, Styled, Text, TextStyled, View,
    palette, text,
};
use rikka_widgets::{icon, row, spacer};

pub(crate) const LINE: f32 = 2.0;

const GLYPH: f32 = 14.0;
const GLYPH_GAP: f32 = 6.0;

pub(super) struct Header {
    symbol: Option<MaterialSymbol>,
    title: Label,
    tint: Option<Color>,
    trailing: Option<Element>,
}

#[must_use]
pub(crate) fn header(symbol: Option<MaterialSymbol>, title: impl Into<Label>) -> Header {
    Header {
        symbol,
        title: title.into(),
        tint: None,
        trailing: None,
    }
}

impl Header {
    #[must_use]
    pub(crate) fn tint(mut self, tint: Color) -> Header {
        self.tint = Some(tint);
        self
    }

    #[must_use]
    pub(crate) fn trailing(self, trailing: impl Into<Label>) -> Header {
        self.beside(figures(trailing).caption().muted().no_wrap().shrink(1.0))
    }

    #[must_use]
    pub(crate) fn beside(mut self, trailing: impl View) -> Header {
        self.trailing = Some(trailing.build());
        self
    }
}

impl View for Header {
    fn build(self) -> Element {
        let tint = self.tint.unwrap_or_else(|| palette().accent);
        row! {
            if let Some(symbol) = self.symbol {
                icon(symbol).extent(Length::px(GLYPH)).filled().color(tint);
            }
            text(self.title)
                .caption()
                .semibold()
                .color(tint)
                .no_wrap()
                .shrink(1.0);
            if let Some(trailing) = self.trailing {
                spacer().grow(1.0).min_width(Space::SNUG.px());
                trailing;
            }
        }
        .gap(GLYPH_GAP)
        .align_items(Align::Center)
        .fill_width()
        .build()
    }
}

#[must_use]
pub(super) fn figures(content: impl Into<Label>) -> Text {
    text(content).family(Family::Monospace)
}

#[must_use]
pub(crate) fn display(content: impl Into<Label>, size: f32) -> Text {
    text(content)
        .text_size(size)
        .light()
        .line_height(size)
        .no_wrap()
}
