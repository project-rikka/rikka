use crate::prelude::*;
use rikka_widgets::{column, row};

const GLYPH: f32 = 20.0;

const GAP: f32 = 12.0;

const INDENT: f32 = GLYPH + GAP;

pub(crate) fn header(
    ready: bool,
    name: impl Into<Label>,
    detail: Option<Element>,
    trailing: Vec<Element>,
) -> impl View {
    row! {
        icon(match ready {
            true => MaterialSymbol::CHECK_CIRCLE,
            false => MaterialSymbol::CIRCLE,
        })
        .extent(Length::px(GLYPH))
        .color(match ready {
            true => palette().accent,
            false => palette().text_muted,
        });
        column! {
            text(name).label().medium();
            detail;
        }
        .gap(Space::HAIR)
        .grow(1.0);
        for beside in trailing {
            beside;
        }
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
}

pub(crate) fn indented(rows: impl View) -> impl View {
    container(rows)
        .padding(Inset::edges(0.0, 0.0, 0.0, INDENT))
        .fill_width()
}

pub(crate) fn secret<M>(
    typed: &TextDocument,
    revealed: bool,
    commit: Handler,
    reveal: impl IntoHandler<(), (), M>,
) -> impl View {
    row! {
        text_field(typed)
            .secret(!revealed)
            .placeholder(tr!("llm-settings.key-placeholder"))
            .on_change(|| {})
            .on_blur(commit.clone())
            .on_submit(commit)
            .grow(1.0);
        button(
            icon(match revealed {
                true => MaterialSymbol::VISIBILITY_OFF,
                false => MaterialSymbol::VISIBILITY,
            })
            .extent(Length::px(18.0)),
        )
        .icon_only()
        .ghost()
        .on_press(reveal);
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width()
}

pub(crate) fn muted(label: impl Into<Label>) -> Text {
    text(label).caption().muted()
}

pub(crate) fn asking(label: impl Into<Label>) -> impl View {
    row! {
        spinner().extent(Length::px(14.0));
        muted(label);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}
