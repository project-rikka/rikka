use rikka_theme::Corner;
use rikka_types::Color;
use rikka_ui::{
    Align, Element, Handler, Inset, IntoHandler, Justify, Label, Length, MaterialSymbol, Space,
    Styled, TextColor, TextStyled, View, palette, text,
};
use rikka_widgets::{
    ControlSize, Flex, button, column, column_of, container, icon, row_of, spacer,
};
use shell_kit::clip;

use crate::centre::detail::Detail;
use crate::centre::state::Centre;

pub(crate) const RADIUS: f32 = Corner::Panel.radius();

pub(crate) const PADDING: f32 = 6.0;

pub(crate) const ICON: f32 = 20.0;

pub(crate) const ROW: f32 = 40.0;

pub(crate) const INSET: f32 = 8.0;

pub(crate) const DETAIL: f32 = 11.5;

pub(crate) fn tile(rows: Vec<Element>) -> impl View {
    container(
        column_of(rows)
            .gap(Space::TIGHT)
            .align_items(Align::Stretch)
            .fill_width(),
    )
    .background(palette().surface)
    .radius(RADIUS)
    .padding(Space::TIGHT)
    .fill_width()
    .resizing()
}

#[must_use]
pub(crate) fn line(symbol: MaterialSymbol, title: Label) -> Line {
    Line {
        symbol,
        tint: None,
        title,
        detail: None,
        trailing: Vec::new(),
        press: None,
    }
}

pub(crate) struct Line {
    symbol: MaterialSymbol,
    tint: Option<Color>,
    title: Label,
    detail: Option<Label>,
    trailing: Vec<Element>,
    press: Option<Handler>,
}

impl Line {
    #[must_use]
    pub(crate) fn detail(mut self, detail: Option<Label>) -> Self {
        self.detail = detail;
        self
    }

    #[must_use]
    pub(crate) fn tint(mut self, tint: Color) -> Self {
        self.tint = Some(tint);
        self
    }

    #[must_use]
    pub(crate) fn with(mut self, trailing: impl View) -> Self {
        self.trailing.push(trailing.build());
        self
    }

    #[must_use]
    pub(crate) fn on_press<Marker>(mut self, press: impl IntoHandler<(), (), Marker>) -> Self {
        self.press = Some(press.into_handler());
        self
    }
}

impl View for Line {
    fn build(self) -> Element {
        let palette = palette();
        let names = column! {
            text(self.title).label().no_wrap();
            if let Some(detail) = self.detail {
                text(detail).caption().muted().no_wrap();
            }
        };

        let mut children = vec![
            icon(self.symbol)
                .extent(Length::px(ICON))
                .color(self.tint.unwrap_or(palette.text))
                .build(),
            clip(names).build(),
        ];
        children.extend(self.trailing);

        let mut line = container(strip(children))
            .radius(RADIUS - PADDING)
            .fill_width();
        if let Some(press) = self.press {
            line = line.on_press(press).focusless();
        }
        line.build()
    }
}

pub(crate) fn strip(children: Vec<Element>) -> Flex {
    row_of(children)
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .justify(Justify::Start)
        .height(ROW)
        .fill_width()
        .padding(Inset::xy(INSET, 0.0))
}

pub(crate) fn choice<Marker>(
    symbol: MaterialSymbol,
    label: Label,
    detail: Option<Label>,
    chosen: bool,
    press: impl IntoHandler<(), (), Marker>,
) -> Element {
    let accent = palette().accent;
    let line = line(symbol, label).detail(detail).on_press(press);
    match chosen {
        true => line.tint(accent).with(
            icon(MaterialSymbol::CHECK)
                .extent(Length::px(ICON))
                .color(accent),
        ),
        false => line,
    }
    .build()
}

pub(crate) fn more(page: Detail) -> impl View {
    let muted = palette().text_muted;
    container(strip(vec![
        text(page.settings()).label().color(muted).build(),
        spacer().grow(1.0).build(),
        icon(MaterialSymbol::CHEVRON_RIGHT)
            .extent(Length::px(ICON))
            .color(muted)
            .build(),
    ]))
    .radius(RADIUS - PADDING)
    .fill_width()
    .on_press(move |centre: &mut Centre| centre.open_settings(page.link()))
    .focusless()
}

pub(crate) fn caption(label: Label) -> impl View {
    text(label)
        .caption()
        .muted()
        .medium()
        .margin(Inset::edges(INSET, INSET, 2.0, INSET))
}

pub(crate) fn refusal(reason: &str) -> impl View {
    text(reason)
        .caption()
        .color(TextColor::Danger)
        .margin(Inset::edges(2.0, INSET, 4.0, INSET))
}

pub(super) fn symbol_button<Marker>(
    symbol: MaterialSymbol,
    press: impl IntoHandler<(), (), Marker>,
) -> impl View {
    button(icon(symbol).extent(Length::px(ICON)).color(palette().text))
        .ghost()
        .icon_only()
        .control_size(ControlSize::Medium)
        .on_press(press)
}

pub(crate) fn round<Marker>(
    symbol: MaterialSymbol,
    press: impl IntoHandler<(), (), Marker>,
) -> impl View {
    let palette = palette();
    let side = ControlSize::Medium.height();
    container(icon(symbol).extent(Length::px(ICON)).color(palette.text))
        .background(palette.surface)
        .radius(side / 2.0)
        .width(side)
        .height(side)
        .center()
        .on_press(press)
}
