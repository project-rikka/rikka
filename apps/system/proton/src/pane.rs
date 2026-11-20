use std::ops::RangeInclusive;

use proton_session::ErrorBanner;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

pub(crate) const SIDEBAR: f32 = 232.0;
pub(crate) const LISTING: f32 = 372.0;

pub(crate) fn input(label: impl Into<Label>, control: impl View) -> Flex {
    column! {
        text(label).caption().muted().no_wrap();
        control;
    }
    .gap(Space::TIGHT)
    .fill_width()
}

pub(crate) fn prose(document: &TextDocument, heights: RangeInclusive<f32>) -> Container {
    framed(
        text_editor(document)
            .family(Family::parse("sans-serif"))
            .fits_text(heights),
    )
}

pub(crate) fn code(document: &TextDocument, heights: RangeInclusive<f32>) -> Container {
    framed(text_editor(document).fits_text(heights))
}

fn framed(editor: TextEditor) -> Container {
    container(editor.gutter(false).fill_width())
        .background(palette().surface_variant)
        .border(1.0, palette().outline)
        .radius(Corner::Control)
        .fill_width()
}

pub(crate) fn heading(label: impl Into<Label>) -> Text {
    text(label).label().semibold().no_wrap()
}

pub(crate) fn toolbar(content: impl View) -> Container {
    container(content)
        .padding(Inset::xy(12.0, 8.0))
        .fill_width()
}

pub(crate) fn nothing(symbol: MaterialSymbol, label: impl Into<Label>) -> Container {
    container(
        column! {
            icon(symbol).extent(Length::px(34.0)).color(palette().text_muted);
            text(label)
                .label()
                .muted()
                .align(TextAlign::Center)
                .max_width(360.0);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center),
    )
    .center()
    .fill()
}

pub(crate) fn working(label: impl Into<Label>) -> Container {
    container(
        row! {
            spinner().extent(Length::px(15.0));
            text(label).caption().muted().no_wrap();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
    .center()
    .fill()
}

pub(crate) fn error_banner<T: 'static>(
    error: &ErrorBanner,
    of: fn(&mut T) -> &mut ErrorBanner,
) -> Option<Container> {
    let message = error.message()?;
    Some(
        container(
            row! {
                icon(MaterialSymbol::ERROR)
                    .extent(Length::px(16.0))
                    .color(palette().danger);
                text(message.to_owned()).caption().grow(1.0);
                button(icon(MaterialSymbol::CLOSE).extent(Length::px(14.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(move |owner: &mut T| of(owner).clear());
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width(),
        )
        .background(palette().danger.with_alpha(0.12))
        .radius(Corner::Panel)
        .padding(Inset::xy(12.0, 6.0))
        .margin(Inset::xy(12.0, 6.0))
        .fill_width(),
    )
}

pub(crate) fn badge(label: impl Into<Label>, tint: Color) -> Container {
    container(text(label).caption().color(tint).no_wrap())
        .background(tint.with_alpha(0.14))
        .radius(Corner::Item)
        .padding(Inset::xy(6.0, 1.0))
}

pub(crate) fn avatar(initial: &str, tint: Color) -> Container {
    container(text(initial.to_owned()).caption().color(tint))
        .background(tint.with_alpha(0.18))
        .radius(16.0)
        .size(32.0, 32.0)
        .center()
}

pub(crate) fn fact(name: impl Into<Label>, value: impl Into<Label>) -> impl View {
    row! {
        text(name).caption().muted().width(148.0);
        text(value).caption().grow(1.0);
    }
    .gap(Space::SNUG)
    .fill_width()
}

pub(crate) fn header<Marker>(
    title: impl Into<Label>,
    back: impl rikka_ui::IntoHandler<(), (), Marker>,
    trailing: impl View,
) -> Container {
    toolbar(
        row! {
            button(icon(MaterialSymbol::ARROW_BACK).extent(Length::px(15.0)))
                .ghost()
                .small()
                .icon_only()
                .on_press(back);
            text(title).label().semibold().grow(1.0);
            trailing;
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
}

pub(crate) fn labelled(symbol: MaterialSymbol, label: impl Into<Label>) -> impl View {
    row! {
        icon(symbol).extent(Length::px(15.0));
        text(label).caption().no_wrap();
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}
