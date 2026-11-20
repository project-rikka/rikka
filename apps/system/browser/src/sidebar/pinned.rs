use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::menus::{Menus, Subject};
use crate::shell::Shell;
use crate::sidebar::face::Face;
use crate::sidebar::look::{FAVICON, GAP, INDENT, PITCH, ROW, folder_symbol};
use crate::sidebar::rename;
use crate::sidebar::row::{Line, RowTarget, row};
use crate::sidebar::state::Renaming;
use crate::widgets::{DragItem, DropLayout, DropTarget, drop_list, glyph_button, grip};
use browser_model::{
    FolderId, Line as Placed, PinPlace, PinnedItem, PinnedTabId, Row, Slot, Target,
};

pub(crate) fn view(shell: &Shell) -> Element {
    let model = shell.model();
    let Some(space) = model.window(shell.id()).and_then(|layout| layout.space()) else {
        return Element::empty();
    };
    let lines = model.pinned.lines(space);
    let header = row! {
        text(tr!("browser.sidebar.pinned")).caption().muted().semibold().grow(1.0);
        glyph_button(MaterialSymbol::CREATE_NEW_FOLDER, 16.0)
            .on_press(|shell: &mut Shell| shell.new_folder(None));
    }
    .align_items(Align::Center)
    .padding(Inset::edges(
        Space::SNUG,
        Space::SNUG,
        Space::NONE,
        Space::SNUG,
    ))
    .height(ROW)
    .fill_width();
    let rows = lines.iter().map(|placed| {
        let line = match placed.item {
            PinnedItem::Folder(id) => {
                component_with(|shell: &Shell| shell, (id, placed.depth), folder)
            }
            PinnedItem::Tab(id) => component_with(|shell: &Shell| shell, (id, placed.depth), pin),
        };
        animated_position(line).keyed(placed.item)
    });
    let folders = lines
        .iter()
        .map(|placed| placed.folder().is_some())
        .collect();
    let count = lines.len();
    column! {
        header;
        drop_list(
            DropLayout::Rows {
                pitch: PITCH,
                folders,
            },
            count,
            column_of(rows).gap(GAP).fill_width(),
        )
        .on_drop(move |shell: &mut Shell, row: Row, target: DropTarget| {
            shell.dropped(row, Target::Pinned(place_for(&lines, target)));
        })
        .min_height(ROW / 2.0)
        .fill_width();
    }
    .fill_width()
    .any()
}

pub(crate) fn place_for(lines: &[Placed], target: DropTarget) -> PinPlace {
    match lines.get(target.index) {
        Some(line) if target.into && line.folder().is_some() => PinPlace {
            parent: line.folder(),
            index: 0,
        },
        Some(line) => line.place,
        None => PinPlace {
            parent: None,
            index: lines.iter().filter(|line| line.depth == 0).count(),
        },
    }
}

fn pin(shell: &Shell, &(pin, depth): &(PinnedTabId, usize)) -> Element {
    let Some(face) = Face::for_slot(&shell.model(), shell.id(), Slot::Pinned(pin)) else {
        return Element::empty();
    };
    row(Line {
        face,
        depth,
        opens: RowTarget::Slot(Slot::Pinned(pin)),
        subject: Subject::Pin(pin),
        row: Row::Pin(PinnedItem::Tab(pin)),
        tears_off: false,
    })
}

fn folder(shell: &Shell, &(folder, depth): &(FolderId, usize)) -> Element {
    let model = shell.model();
    let Some(folder_entry) = model.pinned.folder(folder) else {
        return Element::empty();
    };
    let renaming = shell.sidebar().renaming() == Some(Renaming::Folder(folder));
    let name = match renaming {
        true => rename::field(shell),
        false => text(folder_entry.name.clone())
            .label()
            .no_wrap()
            .grow(1.0)
            .min_width(0.0)
            .any(),
    };
    let chevron = match folder_entry.open {
        true => MaterialSymbol::EXPAND_MORE,
        false => MaterialSymbol::CHEVRON_RIGHT,
    };
    let symbol = folder_symbol(folder_entry.open);
    let body = row! {
        spacer().width(depth as f32 * INDENT);
        icon(chevron).extent(Length::px(FAVICON)).color(palette().text_muted);
        icon(symbol).extent(Length::px(FAVICON)).color(palette().text_muted);
        name;
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .fill();
    grip(body)
        .on_press(move |shell: &mut Shell| shell.toggle_folder(folder))
        .aim(move |menus: &mut Menus| menus.aim(Subject::Folder(folder)))
        .drag_item(DragItem {
            row: Row::Pin(PinnedItem::Folder(folder)),
            symbol: MaterialSymbol::FOLDER,
            label: folder_entry.name.clone(),
        })
        .height(ROW)
        .fill_width()
        .any()
}
