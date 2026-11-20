use rikka_text::Align as TextAlign;
use rikka_types::Color;
use rikka_ui::text::{Label, TextStyled, text};
use rikka_ui::{Align, Element, IconSource, Length, Space, Style, Styled, View, palette};

use crate::blocks::container::container;
use crate::blocks::flex::column_of;
use crate::media::icon::icon;

const GLYPH: f32 = 40.0;
const MEASURE: f32 = 360.0;

#[must_use]
pub struct StatusPage {
    symbol: IconSource,
    message: Label,
    tint: Option<Color>,
    action: Option<Element>,
    style: Style,
}

pub fn status_page(symbol: impl Into<IconSource>, message: impl Into<Label>) -> StatusPage {
    StatusPage {
        symbol: symbol.into(),
        message: message.into(),
        tint: None,
        action: None,
        style: Style::default(),
    }
}

impl StatusPage {
    pub fn tint(mut self, tint: Color) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn action(mut self, action: impl View) -> Self {
        self.action = Some(action.build());
        self
    }
}

impl View for StatusPage {
    fn build(self) -> Element {
        let tint = self.tint.unwrap_or_else(|| palette().text_muted);
        let mut blocks = vec![
            icon(self.symbol)
                .extent(Length::px(GLYPH))
                .color(tint)
                .build(),
            text(self.message)
                .label()
                .muted()
                .align(TextAlign::Center)
                .max_width(MEASURE)
                .build(),
        ];
        blocks.extend(self.action);
        container(
            column_of(blocks)
                .gap(Space::NORMAL)
                .align_items(Align::Center),
        )
        .center()
        .padding(Space::WIDE)
        .styled(self.style)
        .build()
    }
}

impl std::fmt::Debug for StatusPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StatusPage")
            .field("symbol", &self.symbol)
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}

impl Styled for StatusPage {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
