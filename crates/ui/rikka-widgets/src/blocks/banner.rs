use rikka_symbols::MaterialSymbol;
use rikka_types::Color;
use rikka_ui::text::{Label, TextStyled, text};
use rikka_ui::{
    Align, Element, IconSource, IntoHandler, Length, Space, Style, Styled, View, palette,
};

use crate::blocks::flex::row_of;
use crate::controls::button::{Button, button};
use crate::media::icon::icon;

const GLYPH: f32 = 16.0;

#[must_use]
pub struct Banner {
    symbol: IconSource,
    message: Label,
    tint: Option<Color>,
    action: Option<Button>,
    dismiss: Option<Button>,
    style: Style,
}

pub fn banner(message: impl Into<Label>) -> Banner {
    Banner {
        symbol: MaterialSymbol::ERROR.into(),
        message: message.into(),
        tint: None,
        action: None,
        dismiss: None,
        style: Style::default(),
    }
}

impl Banner {
    pub fn symbol(mut self, symbol: impl Into<IconSource>) -> Self {
        self.symbol = symbol.into();
        self
    }

    pub fn tint(mut self, tint: Color) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn on_action<Marker>(
        mut self,
        label: impl Into<Label>,
        act: impl IntoHandler<(), (), Marker>,
    ) -> Self {
        self.action = Some(
            button(text(label).caption())
                .secondary()
                .small()
                .on_press(act),
        );
        self
    }

    pub fn on_dismiss<Marker>(
        mut self,
        label: impl Into<Label>,
        dismiss: impl IntoHandler<(), (), Marker>,
    ) -> Self {
        self.dismiss = Some(
            button(text(label).caption())
                .ghost()
                .small()
                .on_press(dismiss),
        );
        self
    }
}

impl View for Banner {
    fn build(self) -> Element {
        let tint = self.tint.unwrap_or_else(|| palette().danger);
        let mut parts = vec![
            icon(self.symbol)
                .extent(Length::px(GLYPH))
                .color(tint)
                .build(),
            text(self.message).caption().grow(1.0).build(),
        ];
        parts.extend(self.action.map(View::build));
        parts.extend(self.dismiss.map(View::build));
        row_of(parts)
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .styled(self.style)
            .build()
    }
}

impl std::fmt::Debug for Banner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Banner")
            .field("message", &self.message)
            .field("action", &self.action.is_some())
            .field("dismissible", &self.dismiss.is_some())
            .finish_non_exhaustive()
    }
}

impl Styled for Banner {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
