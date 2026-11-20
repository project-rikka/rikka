use std::fmt::Display;

use rikka_theme::Corner;
use rikka_types::{Key, Modifiers};
use rikka_ui::{
    Align, Element, Handler, Inset, IntoHandler, Justify, Label, Length, MaterialSymbol, Space,
    Styled, TextStyled, View, palette, text,
};
use rikka_widgets::{ControlSize, button, column, container, icon, row, rule, spacer, spinner};

use crate::widgets::floating;
use crate::widgets::spot::Spot;

pub const EDITOR_WIDTH: f32 = 980.0;
pub const EDITOR_EDGE: f32 = 14.0;
pub const EDITOR_GAP: f32 = 10.0;
pub const EDITOR_ICON: f32 = 20.0;

const DOWN: f32 = 6.0;
const BORDER: f32 = 1.0;

#[derive(Debug)]
pub struct EditorWindow {
    spot: Spot,
    symbol: MaterialSymbol,
    title: Label,
    close: Handler,
    on_key: Handler<(Key, Modifiers), bool>,
    tools: Element,
    body: Element,
}

#[must_use]
pub fn editor_window<Marker>(
    spot: &Spot,
    symbol: MaterialSymbol,
    title: Label,
    close: impl IntoHandler<(), (), Marker>,
) -> EditorWindow {
    EditorWindow {
        spot: spot.clone(),
        symbol,
        title,
        close: close.into_handler(),
        on_key: Handler::new(|_: Key, _: Modifiers| false),
        tools: Element::empty(),
        body: Element::empty(),
    }
}

impl EditorWindow {
    #[must_use]
    pub fn on_key<Marker>(
        mut self,
        on_key: impl IntoHandler<(Key, Modifiers), bool, Marker>,
    ) -> Self {
        self.on_key = on_key.into_handler();
        self
    }

    #[must_use]
    pub fn tools(mut self, tools: impl View) -> Self {
        self.tools = tools.build();
        self
    }

    #[must_use]
    pub fn body(mut self, body: impl View) -> Self {
        self.body = body.build();
        self
    }
}

impl View for EditorWindow {
    fn build(self) -> Element {
        let EditorWindow {
            spot,
            symbol,
            title,
            close,
            on_key,
            tools,
            body,
        } = self;
        let palette = palette();
        let window = column! {
            heading(symbol, title, close.clone());
            rule();
            tools;
            rule();
            body
        }
        .fill_width();

        let framed = container(window)
            .background(palette.background)
            .border(BORDER, palette.outline)
            .radius(Corner::Window)
            .glass();

        floating(&spot, framed, close)
            .on_key(on_key)
            .width(EDITOR_WIDTH)
            .down(DOWN / 100.0)
            .build()
    }
}

#[must_use]
pub fn editor_loading(height: f32) -> impl View {
    column! { spinner().extent(Length::px(EDITOR_ICON)).color(palette().accent) }
        .align_items(Align::Center)
        .justify(Justify::Center)
        .height(height)
        .fill_width()
}

#[must_use]
pub fn editor_error(trouble: &impl Display, height: f32) -> impl View {
    column! {
        icon(MaterialSymbol::ERROR)
            .extent(Length::px(EDITOR_ICON))
            .color(palette().danger);
        text(trouble.to_string()).caption().muted()
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .justify(Justify::Center)
    .height(height)
    .fill_width()
    .padding(Space::NORMAL)
}

fn heading(symbol: MaterialSymbol, title: Label, close: Handler) -> impl View {
    row! {
        icon(symbol)
            .extent(Length::px(EDITOR_ICON))
            .color(palette().accent);
        text(title).label();
        spacer().grow(1.0);
        button(icon(MaterialSymbol::CLOSE).extent(Length::px(EDITOR_ICON)))
            .ghost()
            .control_size(ControlSize::Small)
            .icon_only()
            .on_press(close)
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .padding(Inset::xy(EDITOR_EDGE, EDITOR_GAP))
    .fill_width()
}
