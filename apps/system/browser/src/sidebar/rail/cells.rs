use rikka_app::prelude::*;
use rikka_ui::DragEnd;

use crate::memory_saver::dimmed;
use crate::menus::{Menus, Subject};
use crate::shell::Shell;
use crate::sidebar::face::Face;
use crate::sidebar::look::folder_symbol;
use crate::sidebar::rail::place::{CELL, GLYPH, RailCell, key_of};
use crate::sidebar::row::RowTarget;
use crate::widgets::{DragItem, GripView, favicon, grip};
use browser_model::{FolderId, PinnedItem, Row, Slot};

pub(super) fn row(shell: &Shell, row: &Row) -> Element {
    let row = *row;
    let (opens, subject) = match row {
        Row::Tab(tab) => (RowTarget::Tab(tab), Subject::Tab(tab)),
        Row::Pin(PinnedItem::Tab(pin)) => (RowTarget::Slot(Slot::Pinned(pin)), Subject::Pin(pin)),
        Row::Favorite(favorite) => (
            RowTarget::Slot(Slot::Favorite(favorite)),
            Subject::Favorite(favorite),
        ),
        Row::Pin(PinnedItem::Folder(folder)) => return folder_cell(shell, folder),
    };
    let model = shell.model();
    let face = match opens {
        RowTarget::Tab(tab) => Face::for_open_tab(&model, shell.id(), tab),
        RowTarget::Slot(slot) => Face::for_slot(&model, shell.id(), slot),
    };
    let Some(face) = face else {
        return Element::empty();
    };
    let mark = match face.loading {
        true => spinner().extent(Length::px(GLYPH)).any(),
        false => dimmed(
            favicon(face.favicon.as_ref(), face.symbol, GLYPH),
            face.discarded,
        ),
    };
    let mut gripped = cell(mark, face.title.clone())
        .highlighted(face.lit)
        .on_press(move |shell: &mut Shell| match opens {
            RowTarget::Tab(tab) => shell.select(tab),
            RowTarget::Slot(slot) => shell.select_slot(slot),
        })
        .aim(move |menus: &mut Menus| menus.aim(subject.clone()))
        .drag_item(DragItem {
            row,
            symbol: face.symbol,
            label: face.title,
        });
    if let Some(tab) = face.tab {
        gripped = gripped.middle(move |shell: &mut Shell| shell.close(tab));
    }
    if let RowTarget::Tab(tab) = opens {
        gripped = gripped.ended(move |shell: &mut Shell, end: DragEnd| {
            if end == DragEnd::Nowhere {
                shell.tear_off(tab);
            }
        });
    }
    gripped.any()
}

fn folder_cell(shell: &Shell, folder: FolderId) -> Element {
    let model = shell.model();
    let Some(folder_entry) = model.pinned.folder(folder) else {
        return Element::empty();
    };
    let row = Row::Pin(PinnedItem::Folder(folder));
    cell(
        glyph(folder_symbol(folder_entry.open), palette().text_muted),
        folder_entry.name.clone(),
    )
    .on_press(move |shell: &mut Shell| shell.toggle_folder(folder))
    .aim(move |menus: &mut Menus| menus.aim(Subject::Folder(folder)))
    .drag_item(DragItem {
        row,
        symbol: MaterialSymbol::FOLDER,
        label: folder_entry.name.clone(),
    })
    .any()
}

pub(super) fn new_tab() -> Element {
    cell(
        glyph(MaterialSymbol::ADD, palette().text_muted),
        tr!("browser.sidebar.new-tab").to_string(),
    )
    .on_press(|shell: &mut Shell| shell.new_tab())
    .build()
    .keyed(key_of(RailCell::NewTab))
}

pub(crate) fn cell(mark: impl View, label: String) -> GripView {
    grip(container(mark).center().fill())
        .tip(label, TooltipSide::After)
        .size(Length::px(CELL), Length::px(CELL))
}

pub(super) fn glyph(symbol: MaterialSymbol, tint: Color) -> Element {
    icon(symbol).extent(Length::px(GLYPH)).color(tint).any()
}
