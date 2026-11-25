mod lift;

use rikka_app::prelude::*;
use rikka_ui::IntoHandler;
use rikka_widgets::row;

pub(crate) use lift::lift;

pub(crate) fn chip(label: impl Into<Label>, tint: Color) -> Element {
    container(text(label).caption().semibold().color(tint).no_wrap())
        .padding(Inset::xy(Space::SNUG, Space::HAIR))
        .radius(Corner::Control)
        .background(tint.with_alpha(0.14))
        .any()
}

pub(crate) fn stat(symbol: MaterialSymbol, value: impl Into<Label>) -> Element {
    row! {
        icon(symbol).extent(Length::px(15.0)).color(palette().text_muted);
        text(value).caption().muted().no_wrap();
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .any()
}

pub(crate) fn icon_button<Marker>(
    symbol: MaterialSymbol,
    tooltip: impl Into<Label>,
    press: impl IntoHandler<(), (), Marker>,
) -> Element {
    button(icon(symbol).extent(Length::px(18.0)))
        .ghost()
        .icon_only()
        .on_press(press)
        .tooltip(tooltip.into())
        .any()
}

pub(crate) fn labelled(symbol: MaterialSymbol, label: impl Into<Label>) -> impl View {
    row! {
        icon(symbol).extent(Length::px(18.0));
        text(label).no_wrap();
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

pub(crate) fn heading(title: impl Into<Label>) -> Element {
    text(title).heading().bold().any()
}
