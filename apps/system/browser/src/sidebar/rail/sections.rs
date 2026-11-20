use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::shell::Shell;
use crate::sidebar::look::{EDGE, rule};
use crate::sidebar::pinned::place_for;
use crate::sidebar::rail::cells;
use crate::sidebar::rail::place::{CELL, PITCH, RailCell, key_of};
use crate::split::{Entries, Entry, rail_cell};
use crate::widgets::{DropLayout, DropListView, DropTarget, drop_list};
use browser_model::{Row, Target};

pub(super) fn favorites(shell: &Shell) -> Element {
    if shell.private() {
        return Element::empty();
    }
    let rows: Vec<Row> = shell
        .model()
        .favorites_here(shell.id())
        .iter()
        .map(|favorite| Row::Favorite(favorite.id))
        .collect();
    let count = rows.len();
    let cells = rows.into_iter().map(|row| {
        animated_position(component_with(|shell: &Shell| shell, row, cells::row))
            .keyed(key_of(RailCell::Row(row)))
    });
    let list = drop_list(
        DropLayout::Rows {
            pitch: PITCH,
            folders: Vec::new(),
        },
        count,
        column_of(cells).gap(Space::HAIR),
    )
    .on_drop(|shell: &mut Shell, row: Row, target: DropTarget| {
        shell.dropped(row, Target::Favorites(target.index));
    });
    section(list, count)
}

pub(super) fn pinned(shell: &Shell) -> Element {
    let model = shell.model();
    let Some(space) = model.window(shell.id()).and_then(|layout| layout.space()) else {
        return Element::empty();
    };
    let lines = model.pinned.lines(space);
    let cells = lines.iter().map(|line| {
        let row = Row::Pin(line.item);
        animated_position(component_with(|shell: &Shell| shell, row, cells::row))
            .keyed(key_of(RailCell::Row(row)))
    });
    let folders = lines.iter().map(|line| line.folder().is_some()).collect();
    let count = lines.len();
    let list = drop_list(
        DropLayout::Rows {
            pitch: PITCH,
            folders,
        },
        count,
        column_of(cells).gap(Space::HAIR),
    )
    .on_drop(move |shell: &mut Shell, row: Row, target: DropTarget| {
        shell.dropped(row, Target::Pinned(place_for(&lines, target)));
    });
    section(list, count)
}

pub(super) fn today(shell: &Shell) -> Element {
    let entries = Entries::collect(&shell.model(), shell.id()).shown;
    let count = entries.len();
    let cells = entries.into_iter().map(|entry| match entry {
        Entry::Tab(tab) => {
            let row = Row::Tab(tab);
            animated_position(component_with(|shell: &Shell| shell, row, cells::row))
                .keyed(key_of(RailCell::Row(row)))
        }
        Entry::Group(group) => {
            animated_position(component_with(|shell: &Shell| shell, group, rail_cell))
                .keyed(key_of(RailCell::Group(group)))
        }
    });
    drop_list(
        DropLayout::Rows {
            pitch: PITCH,
            folders: Vec::new(),
        },
        count,
        column_of(cells).gap(Space::HAIR),
    )
    .on_drop(|shell: &mut Shell, row: Row, target: DropTarget| {
        shell.dropped_in_today(row, target.index);
    })
    .min_height(CELL)
    .width(Length::px(CELL))
    .any()
}

fn section(list: DropListView, count: usize) -> Element {
    column! {
        list.min_height(EDGE).width(Length::px(CELL));
        (count > 0).then(rule);
    }
    .align_items(Align::Center)
    .fill_width()
    .any()
}
