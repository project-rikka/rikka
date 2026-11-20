use cut_project::MediaId;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::draft::{Busy, Entry, Link, Relinking};
use crate::cut::Cut;
use crate::view::STAGE;
use crate::wording;

const WIDTH: f32 = 560.0;
const LIST: f32 = 320.0;
const GLYPH: f32 = 20.0;

pub(crate) fn sheet(relinking: &Relinking) -> Vec<Element> {
    let busy = relinking.busy;
    let found = relinking.relinks().len();
    let entries: Vec<Element> = relinking
        .entries
        .iter()
        .map(|entry| entry_row(entry, busy).build())
        .collect();
    let body = column! {
        scrollable(column_of(entries).gap(Space::NORMAL).fill_width())
            .max_height(LIST)
            .fill_width();
        if let Some(note) = &relinking.note {
            text(note.clone()).caption().muted().fill_width();
        }
    }
    .gap(Space::ROOMY)
    .fill_width();
    dialog(tr!("cut.relink.title"), body)
        .summary(tr!("cut.relink.summary", count = relinking.entries.len()))
        .aside(
            button(text(tr!("cut.relink.search")))
                .ghost()
                .loading(busy == Some(Busy::Searching))
                .enabled(busy.is_none())
                .on_press(Cut::search_media),
        )
        .cancel(tr!("cut.relink.keep_offline"), Cut::close_relink)
        .accept(tr!("cut.relink.apply", count = found), Cut::relink_found)
        .ready(found > 0 && busy.is_none())
        .width(WIDTH)
        .keyed("cut-relink")
        .centered_on(STAGE)
        .with_scrim()
}

fn entry_row(entry: &Entry, busy: Option<Busy>) -> impl View {
    let media: MediaId = entry.media;
    let (glyph, tone) = match entry.link {
        Link::Missing => (MaterialSymbol::LINK_OFF, palette().danger),
        Link::Found(_) => (MaterialSymbol::CHECK_CIRCLE, palette().success),
        Link::Refused { .. } => (MaterialSymbol::ERROR, palette().warning),
    };
    let status = match &entry.link {
        Link::Missing => text(tr!("cut.relink.uses", count = entry.uses))
            .caption()
            .muted(),
        Link::Found(to) => text(tr!(
            "cut.relink.found",
            path = &to.path().display().to_string()
        ))
        .caption()
        .color(palette().success),
        Link::Refused { path, reason } => text(tr!(
            "cut.relink.refused",
            name = &wording::file_name(path),
            reason = &reason.to_string()
        ))
        .caption()
        .color(palette().warning),
    };
    row! {
        icon(glyph).extent(Length::px(GLYPH)).color(tone);
        column! {
            text(entry.original.name()).semibold().no_wrap();
            text(entry.original.path().display().to_string())
                .caption()
                .muted()
                .no_wrap();
            status;
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .shrink(1.0);
        button(text(tr!("cut.relink.locate")))
            .small()
            .loading(busy == Some(Busy::Locating(media)))
            .enabled(busy.is_none())
            .on_press(move |cut: &mut Cut| cut.locate_media(media));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}
