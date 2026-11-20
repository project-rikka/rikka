use rikka_app::prelude::*;
use rikka_format::format_bytes;
use rikka_ui::IntoHandler;
use viewer_media::{Media, Neighbours, Step};

use crate::open_file::OpenFile;
use crate::viewer::Viewer;

const GLYPH: f32 = 18.0;

pub(crate) fn status(
    file: &OpenFile,
    neighbours: Option<&Neighbours>,
    stepping_blocked: bool,
    trailing: Option<Element>,
) -> impl View {
    row! {
        if let Some(neighbours) = neighbours.filter(|neighbours| neighbours.count() > 1) {
            navigation(neighbours, stepping_blocked);
        }
        text(file.name()).label().medium().no_wrap();
        if let Some(media) = file.media() {
            detail(describe(media), palette().text_muted);
        }
        spacer();
        trailing;
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

pub(crate) fn detail(label: impl Into<Label>, color: Color) -> impl View {
    text(label).caption().color(color).no_wrap()
}

pub(crate) fn icon_button<Marker>(
    symbol: MaterialSymbol,
    press: impl IntoHandler<(), (), Marker>,
    enabled: bool,
) -> impl View {
    button(icon(symbol).extent(Length::px(GLYPH)))
        .icon_only()
        .small()
        .enabled(enabled)
        .on_press(press)
}

fn navigation(neighbours: &Neighbours, stepping_blocked: bool) -> impl View {
    let step = |symbol, step: Step| {
        icon_button(
            symbol,
            move |viewer: &mut Viewer| viewer.step(step),
            !stepping_blocked && neighbours.step(step).is_some(),
        )
    };

    row! {
        step(MaterialSymbol::CHEVRON_LEFT, Step::Previous);
        step(MaterialSymbol::CHEVRON_RIGHT, Step::Next);
        detail(
            format!("{} / {}", neighbours.index() + 1, neighbours.count()),
            palette().text_muted,
        );
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

fn describe(media: Media) -> String {
    let name = media.format.name();
    let size = format_bytes(media.bytes);
    match media.extent {
        Some(extent) => format!("{name} · {}×{} · {size}", extent.width, extent.height),
        None => format!("{name} · {size}"),
    }
}
