use browser_store::{ClosedTab, Store};
use jiff::civil::Date;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::internal::days;
use crate::shell::Shell;
use crate::sidebar::titled;
use crate::widgets::{grip, stored_favicon};
use browser_model::TabId;

const GLYPH: f32 = 16.0;
const ROW: f32 = 40.0;
const WIDEST: f32 = 560.0;

pub(crate) fn view(shell: &Shell, tab: TabId) -> Option<Element> {
    let closed = shell.pages().home.closed();
    if closed.is_empty() {
        return None;
    }
    let store = shell.browser().store();
    let today = days::today();
    let list = column! {
        text(tr!("browser.home.closed")).label().semibold().muted();
        column_of(closed.iter().map(|entry| closed_row(entry, tab, &store, today)))
            .gap(Space::HAIR)
            .fill_width();
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .max_width(WIDEST)
    .fill_width()
    .any();
    Some(list)
}

fn closed_row(entry: &ClosedTab, tab: TabId, store: &Store, today: Date) -> Element {
    let when = days::local_time(entry.closed_at)
        .map(|local_time| match local_time.date == today {
            true => local_time.time,
            false => days::day(local_time.date, today),
        })
        .unwrap_or_default();
    let host = crate::address::display_address(&entry.url).host;
    let reopened = entry.clone();
    grip(
        row! {
            stored_favicon(Some(store), &entry.url, MaterialSymbol::PUBLIC, GLYPH);
            text(titled(&entry.title, &entry.url))
                .label()
                .no_wrap()
                .min_width(0.0)
                .grow(1.0);
            text(host).caption().muted().no_wrap();
            text(when).caption().muted().no_wrap();
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .padding(Inset::xy(Space::NORMAL, Space::NONE))
        .fill(),
    )
    .on_press(move |shell: &mut Shell| shell.reopen_from_home(tab, reopened.clone()))
    .height(ROW)
    .fill_width()
    .build()
    .keyed(entry.id)
}
