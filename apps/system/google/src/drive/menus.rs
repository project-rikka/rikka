use rikka_app::prelude::*;

use crate::drive::row::Column;
use crate::drive::{DriveState, Menu};

pub(super) fn items(drive: &DriveState, menu: Menu) -> Vec<MenuItem> {
    match menu {
        Menu::Context(_) if drive.selected().is_empty() => folder_items(drive),
        Menu::Context(_) => selection_items(drive),
        Menu::New => creation_items(drive),
        Menu::Sort => sort_items(drive),
    }
}

fn selection_items(drive: &DriveState) -> Vec<MenuItem> {
    let selected = drive.selected();
    let single = selected.len() == 1;
    let any = |test: fn(&google_drive::File) -> bool| selected.iter().any(|row| test(&row.file));
    let all = |test: fn(&google_drive::File) -> bool| selected.iter().all(|row| test(&row.file));
    if drive.location().is_trash() {
        return vec![
            menu_item(tr!("google.drive.menu.restore"))
                .icon(MaterialSymbol::RESTORE_FROM_TRASH)
                .on_press(DriveState::restore_selected),
            menu_item(tr!("google.drive.menu.delete-forever"))
                .icon(MaterialSymbol::DELETE_FOREVER)
                .on_press(DriveState::discard_selected),
        ];
    }
    let starred = all(|file| file.starred);
    vec![
        menu_item(tr!("google.drive.menu.open"))
            .icon(MaterialSymbol::OPEN_IN_NEW)
            .enabled(single)
            .on_press(DriveState::open_selected),
        menu_item(tr!("google.drive.menu.open-browser"))
            .icon(MaterialSymbol::LANGUAGE)
            .enabled(any(|file| file.web_view_link.is_some()))
            .on_press(DriveState::browse_selected),
        menu_separator(),
        menu_item(tr!("google.drive.menu.download"))
            .icon(MaterialSymbol::DOWNLOAD)
            .enabled(any(|file| !file.is_folder()))
            .on_press(|drive: &mut DriveState| drive.download_selected(false)),
        menu_item(tr!("google.drive.menu.download-pdf"))
            .icon(MaterialSymbol::PICTURE_AS_PDF)
            .enabled(any(google_drive::File::is_native))
            .on_press(|drive: &mut DriveState| drive.download_selected(true)),
        menu_separator(),
        menu_item(tr!("google.drive.menu.rename"))
            .icon(MaterialSymbol::EDIT)
            .enabled(single && all(|file| file.capabilities.can_rename))
            .on_press(DriveState::begin_rename),
        menu_item(match starred {
            true => tr!("google.drive.menu.unstar"),
            false => tr!("google.drive.menu.star"),
        })
        .icon(MaterialSymbol::STAR)
        .on_press(move |drive: &mut DriveState| drive.star_selected(!starred)),
        menu_item(tr!("google.drive.menu.copy-link"))
            .icon(MaterialSymbol::LINK)
            .enabled(any(|file| file.web_view_link.is_some()))
            .on_press(DriveState::copy_link),
        menu_separator(),
        menu_item(tr!("google.drive.menu.trash"))
            .icon(MaterialSymbol::DELETE)
            .enabled(all(|file| file.capabilities.can_trash))
            .on_press(DriveState::discard_selected),
    ]
}

fn folder_items(drive: &DriveState) -> Vec<MenuItem> {
    let mut items = creation_items(drive);
    items.push(menu_separator());
    items.push(
        menu_item(tr!("google.drive.menu.refresh"))
            .icon(MaterialSymbol::REFRESH)
            .on_press(|drive: &mut DriveState| {
                drive.close_menu();
                drive.refresh();
            }),
    );
    if drive.location().is_trash() {
        items.push(
            menu_item(tr!("google.drive.menu.empty-trash"))
                .icon(MaterialSymbol::DELETE_SWEEP)
                .enabled(drive.count() > 0)
                .on_press(DriveState::ask_empty_trash),
        );
    }
    items
}

fn creation_items(drive: &DriveState) -> Vec<MenuItem> {
    let takes = drive.takes_files();
    vec![
        menu_item(tr!("google.drive.menu.new-folder"))
            .icon(MaterialSymbol::CREATE_NEW_FOLDER)
            .enabled(takes)
            .on_press(DriveState::new_folder),
        menu_item(tr!("google.drive.menu.upload"))
            .icon(MaterialSymbol::UPLOAD_FILE)
            .enabled(takes)
            .on_press(DriveState::upload),
    ]
}

fn sort_items(drive: &DriveState) -> Vec<MenuItem> {
    let sorting = drive.sorting();
    let mut items = vec![
        menu_item(tr!("google.drive.sort.listed"))
            .selected(sorting.column.is_none())
            .on_press(|drive: &mut DriveState| {
                drive.close_menu();
                drive.sort_as_listed();
            }),
    ];
    items.extend(Column::ALL.map(|column| {
        menu_item(column.title())
            .selected(sorting.column == Some(column))
            .on_press(move |drive: &mut DriveState| {
                drive.close_menu();
                drive.sort_by(column);
            })
    }));
    items
}
