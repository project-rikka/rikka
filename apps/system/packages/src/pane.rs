use rikka_app::prelude::*;
use rikka_nix::Stage;
use rikka_widgets::{column, row};

pub(crate) const DETAIL: f32 = 396.0;

const SPINNER: f32 = 13.0;

pub(crate) fn panel(content: impl View) -> impl View {
    container(content)
        .surface()
        .radius(Corner::Panel)
        .padding(Space::ROOMY)
        .fill_width()
}

pub(crate) fn strip(
    symbol: MaterialSymbol,
    tint: Color,
    label: impl Into<Label>,
    action: Button,
) -> impl View {
    container(
        row! {
            icon(symbol).extent(Length::px(16.0)).color(tint);
            text(label).caption().grow(1.0);
            action;
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
    .surface()
    .radius(Corner::Panel)
    .padding(Inset::xy(12.0, 8.0))
    .fill_width()
}

pub(crate) fn working(stage: Stage) -> impl View {
    let label = match stage {
        Stage::Requested => tr!("packages.work.authorising"),
        Stage::Idle | Stage::Running => tr!("packages.work.running"),
    };
    row! {
        spinner().extent(Length::px(SPINNER));
        text(label).caption().no_wrap();
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

pub(crate) fn card(content: impl View) -> impl View {
    container(content)
        .surface()
        .radius(Corner::Panel)
        .padding(Space::NORMAL)
        .fill_width()
}

fn detail_row(title: Label, value: String) -> impl View {
    row! {
        text(title).caption().muted().width(112.0).no_wrap();
        text(value).caption().grow(1.0);
    }
    .gap(Space::SNUG)
    .fill_width()
}

pub(crate) fn details(rows: Vec<(Label, String)>) -> impl View {
    column! {
        for (title, value) in rows {
            detail_row(title, value);
        }
    }
    .gap(Space::SNUG)
    .fill_width()
}

pub(crate) fn badge(label: String, tint: Color) -> impl View {
    container(text(label).caption().color(tint).no_wrap())
        .border(1.0, tint.with_alpha(0.5))
        .radius(Corner::Item)
        .padding(Inset::xy(8.0, 2.0))
}
