use rikka_app::prelude::*;
use rikka_widgets::column;

fn stat(caption: Label, value: String) -> impl View {
    column! {
        text(caption).caption().muted();
        text(value).title();
    }
    .gap(Space::HAIR)
    .width(148.0)
}

pub(crate) fn stats(entries: Vec<(Label, String)>) -> impl View {
    row! {
        for (caption, value) in entries {
            stat(caption, value);
        }
    }
    .gap(Space::ROOMY)
    .wrap_items()
}

fn detail(caption: Label, value: String) -> impl View {
    row! {
        text(caption).label().muted().no_wrap();
        spacer();
        text(value).label().no_wrap();
    }
    .gap(Space::NORMAL)
}

pub(crate) fn details(entries: Vec<(Label, String)>) -> impl View {
    column! {
        for (caption, value) in entries {
            detail(caption, value);
        }
    }
    .gap(Space::TIGHT)
    .width(280.0)
}

pub(crate) fn composition(parts: &[(f32, Color)]) -> impl View {
    let total: f32 = parts.iter().map(|(share, _)| share.max(0.0)).sum();
    container(
        row! {
            for &(share, colour) in parts {
                container(spacer())
                    .background(colour)
                    .fill_height()
                    .grow((share.max(0.0) / total.max(1.0)).max(0.001));
            }
        }
        .height(28.0),
    )
    .border(1.0, palette().outline)
    .radius(Corner::Detail)
}
