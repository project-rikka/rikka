use files_convert::Target;
use files_core::{SortKey, ViewMode};
use files_undo::Direction;
use rikka_app::prelude::*;
use rikka_ui::Command as Standard;

use crate::devices::{Command, Devices};
use crate::tabs::Tabs;
use crate::window::menus::{Menu, Menus, Rows};
use crate::window::{WindowState, keys};
use crate::wording;

impl WindowState {
    pub(crate) fn menu_items(&self, menu: Menu) -> Vec<MenuItem> {
        let mut rows = Rows::default();
        match menu {
            Menu::Context { .. } if self.tabs.active().chosen_count() > 0 => {
                self.selection_items(&mut rows);
            }
            Menu::Context { pastable, .. } => self.folder_items(&mut rows, pastable),
            Menu::Places { index, .. } => self.places_items(&mut rows, index),
            Menu::Tools => self.tool_items(&mut rows),
            Menu::Sort => self.sort_items(&mut rows),
            Menu::OpenWith => self.open_with_items(&mut rows),
        }
        rows.finish()
    }

    fn selection_items(&self, rows: &mut Rows) {
        let tab = self.tabs.active();
        let allows = tab.selection_allows();

        rows.command(
            tr!("files.menu.open"),
            MaterialSymbol::OPEN_IN_NEW,
            WindowState::open_selected,
        );
        if tab.any_folder_selected() {
            rows.command(
                tr!("files.menu.open-tab"),
                MaterialSymbol::TAB,
                WindowState::open_selected_tabs,
            );
        }
        if tab.at().is_local() {
            rows.command(
                tr!("files.menu.open-with"),
                MaterialSymbol::APPS,
                |menus: &mut Menus| menus.show(Menu::OpenWith),
            );
        }
        if tab.at().is_local() && tab.chosen_count() == 1 && tab.any_folder_selected() {
            rows.command(
                tr!("files.menu.terminal"),
                MaterialSymbol::TERMINAL,
                WindowState::open_terminal_in_selection,
            );
        }
        if self.wallpaper_source().is_some() {
            rows.command(
                tr!("files.menu.wallpaper"),
                MaterialSymbol::WALLPAPER,
                WindowState::set_wallpaper,
            );
        }
        rows.rule();
        if self.archive_selected().is_some() {
            rows.command(
                tr!("files.menu.extract"),
                MaterialSymbol::UNARCHIVE,
                WindowState::extract,
            );
        }
        if self.packable() {
            rows.command(
                tr!("files.menu.compress"),
                MaterialSymbol::ARCHIVE,
                WindowState::show_compress,
            );
        }
        let conversions = self.conversions();
        if !conversions.is_empty() {
            rows.branch(
                tr!("files.menu.convert"),
                MaterialSymbol::CHANGE_CIRCLE,
                conversion_rows(&conversions),
            );
        }
        rows.rule();
        if allows.restore {
            rows.command(
                tr!("files.menu.restore"),
                MaterialSymbol::RESTORE_FROM_TRASH,
                WindowState::restore,
            );
            rows.rule();
        }
        rows.keyed(
            tr!("files.menu.copy"),
            MaterialSymbol::CONTENT_COPY,
            Standard::Copy.chord().to_string(),
            WindowState::copy,
        );
        if allows.rename {
            rows.keyed(
                tr!("files.menu.cut"),
                MaterialSymbol::CONTENT_CUT,
                Standard::Cut.chord().to_string(),
                WindowState::cut,
            );
        }
        if allows.rename && tab.chosen_count() == 1 {
            rows.keyed(
                tr!("files.menu.rename"),
                MaterialSymbol::DRIVE_FILE_RENAME_OUTLINE,
                Standard::Rename.chord().to_string(),
                |tabs: &mut Tabs| tabs.active_mut().begin_rename(),
            );
        }
        rows.rule();
        self.creation_items(rows);
        if allows.trash {
            rows.keyed(
                tr!("files.menu.trash"),
                MaterialSymbol::DELETE,
                Standard::Delete.chord().to_string(),
                WindowState::trash,
            );
        }
        if allows.delete {
            rows.keyed(
                tr!("files.menu.delete"),
                MaterialSymbol::DELETE_FOREVER,
                keys::label(keys::DELETE_FOREVER),
                WindowState::confirm_delete,
            );
        }
        if tab.any_folder_selected() {
            rows.rule();
            rows.command(
                tr!("files.menu.pin"),
                MaterialSymbol::PUSH_PIN,
                WindowState::pin_selection,
            );
        }
        rows.rule();
        rows.keyed(
            tr!("files.menu.properties"),
            MaterialSymbol::INFO,
            Standard::Properties.chord().to_string(),
            WindowState::show_properties,
        );
    }

    fn folder_items(&self, rows: &mut Rows, pastable: bool) {
        let tab = self.tabs.active();
        let allows = tab.folder_allows();

        self.creation_items(rows);
        if allows.write && pastable {
            rows.keyed(
                tr!("files.menu.paste"),
                MaterialSymbol::CONTENT_PASTE,
                Standard::Paste.chord().to_string(),
                WindowState::paste,
            );
            rows.rule();
        }
        self.reversal_items(rows);
        if allows.empty && !tab.rows().is_empty() {
            rows.command(
                tr!("files.menu.empty-trash"),
                MaterialSymbol::DELETE_FOREVER,
                WindowState::empty_trash,
            );
            rows.rule();
        }
        if tab.at().is_local() {
            rows.command(
                tr!("files.menu.terminal"),
                MaterialSymbol::TERMINAL,
                WindowState::open_terminal_here,
            );
            rows.rule();
        }
        rows.keyed(
            tr!("files.menu.select-all"),
            MaterialSymbol::CHECK,
            Standard::SelectAll.chord().to_string(),
            WindowState::select_all,
        );
        rows.keyed(
            tr!("files.menu.refresh"),
            MaterialSymbol::REFRESH,
            Standard::Refresh.chord().to_string(),
            WindowState::refresh_active,
        );
    }

    fn creation_items(&self, rows: &mut Rows) {
        if !self.tabs.active().folder_allows().create {
            return;
        }
        rows.keyed(
            tr!("files.menu.new-folder"),
            MaterialSymbol::CREATE_NEW_FOLDER,
            keys::label(keys::NEW_FOLDER),
            |window: &mut WindowState| window.create(true),
        );
        rows.command(
            tr!("files.menu.new-file"),
            MaterialSymbol::NOTE_ADD,
            |window: &mut WindowState| window.create(false),
        );
        rows.rule();
    }

    fn tool_items(&self, rows: &mut Rows) {
        let tab = self.tabs.active();

        rows.keyed(
            tr!("files.menu.new-tab"),
            MaterialSymbol::TAB,
            Standard::NewTab.chord().to_string(),
            WindowState::new_tab,
        );
        rows.rule();
        self.reversal_items(rows);
        if tab.folder_allows().create {
            rows.keyed(
                tr!("files.menu.new-folder"),
                MaterialSymbol::CREATE_NEW_FOLDER,
                keys::label(keys::NEW_FOLDER),
                |window: &mut WindowState| window.create(true),
            );
            rows.rule();
        }
        rows.toggle(
            tr!("files.menu.hidden"),
            MaterialSymbol::VISIBILITY,
            tab.shows_hidden(),
            WindowState::toggle_hidden,
        );
        rows.toggle(
            tr!("files.menu.search-contents"),
            MaterialSymbol::SEARCH,
            tab.finding().searches_contents(),
            WindowState::toggle_search_contents,
        );
        rows.rule();
        if tab.at().to_path().is_none() {
            return;
        }
        if self.services.is_pinned(tab.at()) {
            rows.command(
                tr!("files.menu.unpin"),
                MaterialSymbol::DELETE,
                WindowState::unpin_here,
            );
        } else {
            rows.command(
                tr!("files.menu.pin"),
                MaterialSymbol::PUSH_PIN,
                WindowState::pin_here,
            );
        }
    }

    fn reversal_items(&self, rows: &mut Rows) {
        if let Some(kind) = self.reversible(Direction::Undo) {
            rows.keyed(
                wording::undo(kind),
                MaterialSymbol::UNDO,
                Standard::Undo.chord().to_string(),
                WindowState::undo,
            );
        }
        if let Some(kind) = self.reversible(Direction::Redo) {
            rows.keyed(
                wording::redo(kind),
                MaterialSymbol::REDO,
                Standard::Redo.chord().to_string(),
                WindowState::redo,
            );
        }
        rows.rule();
    }

    fn places_items(&self, rows: &mut Rows, index: usize) {
        let Some(row) = self.services.row(index) else {
            return;
        };

        if row.at.is_some() {
            rows.command(
                tr!("files.menu.open"),
                MaterialSymbol::OPEN_IN_NEW,
                move |window: &mut WindowState| window.open_place(index, false),
            );
            rows.command(
                tr!("files.menu.open-tab"),
                MaterialSymbol::TAB,
                move |window: &mut WindowState| window.open_place(index, true),
            );
        }

        if let Some(device) = row.device() {
            rows.rule();
            let id = &device.id;
            if device.actions.unlock {
                rows.command(
                    tr!("files.menu.unlock"),
                    MaterialSymbol::LOCK_OPEN,
                    move |window: &mut WindowState| window.open_place(index, false),
                );
            }
            if device.actions.mount {
                device_command(
                    rows,
                    tr!("files.menu.mount"),
                    MaterialSymbol::HARD_DRIVE,
                    Command::Mount {
                        id: id.clone(),
                        open: true,
                    },
                );
            }
            if device.actions.unmount {
                device_command(
                    rows,
                    tr!("files.menu.unmount"),
                    MaterialSymbol::EJECT,
                    Command::Unmount(id.clone()),
                );
            }
            if device.actions.eject {
                device_command(
                    rows,
                    tr!("files.menu.eject"),
                    MaterialSymbol::EJECT,
                    Command::Eject(id.clone()),
                );
            }
            if device.actions.lock {
                device_command(
                    rows,
                    tr!("files.menu.lock"),
                    MaterialSymbol::LOCK,
                    Command::Lock(id.clone()),
                );
            }
            return;
        }

        let pinned = row.is_pin();
        let Some(at) = row.at else {
            return;
        };
        rows.rule();
        if pinned {
            rows.command(
                tr!("files.menu.unpin"),
                MaterialSymbol::DELETE,
                move |window: &mut WindowState| window.unpin(&at),
            );
        } else if at.is_local() {
            rows.command(
                tr!("files.menu.pin"),
                MaterialSymbol::PUSH_PIN,
                move |window: &mut WindowState| window.pin(&at),
            );
        }
    }

    fn sort_items(&self, rows: &mut Rows) {
        let tab = self.tabs.active();
        for (label, key) in [
            (tr!("files.column.name"), SortKey::Name),
            (tr!("files.column.modified"), SortKey::Modified),
            (tr!("files.column.kind"), SortKey::Kind),
            (tr!("files.column.size"), SortKey::Size),
        ] {
            rows.choice(
                label,
                tab.sorting().key == key,
                move |window: &mut WindowState| window.sort_listing(key),
            );
        }
        rows.rule();
        for mode in [ViewMode::Icons, ViewMode::Details] {
            rows.choice(
                wording::view_mode(mode),
                tab.mode() == mode,
                move |window: &mut WindowState| window.view_as(mode),
            );
        }
    }

    fn open_with_items(&self, rows: &mut Rows) {
        let Some(entry) = self.tabs.active().selected_entries().first().copied() else {
            return;
        };
        let applications = self.services.applications();
        let handlers = applications.for_type(&entry.mime, &self.services.mime);

        for application in &handlers.all {
            let id = application.id.clone();
            rows.choice(
                application.name.as_str(),
                handlers
                    .default
                    .is_some_and(|chosen| chosen.id == application.id),
                move |window: &mut WindowState| window.open_with(&id),
            );
        }
        rows.rule();
        rows.command(
            tr!("files.menu.desktop-default"),
            MaterialSymbol::OPEN_IN_NEW,
            WindowState::open_with_desktop,
        );
    }
}

fn device_command(rows: &mut Rows, label: Label, symbol: MaterialSymbol, command: Command) {
    rows.command(label, symbol, move |devices: &mut Devices| {
        devices.run(command.clone());
    });
}

fn conversion_rows(targets: &[Target]) -> Rows {
    let mut formats = Rows::default();
    let mut shrinks = Rows::default();
    let mut extracts = Rows::default();
    let mut strips = false;
    for &target in targets {
        let rows = match target {
            Target::StripMetadata => {
                strips = true;
                continue;
            }
            Target::Shrink(_) => &mut shrinks,
            Target::Extract(_) => &mut extracts,
            _ => &mut formats,
        };
        rows.item(
            wording::conversion(target),
            move |window: &mut WindowState| window.convert_to(target),
        );
    }
    formats.rule();
    formats.branch(
        tr!("files.convert.shrink"),
        MaterialSymbol::PHOTO_SIZE_SELECT_LARGE,
        shrinks,
    );
    formats.branch(
        tr!("files.convert.extract"),
        MaterialSymbol::MUSIC_NOTE,
        extracts,
    );
    if strips {
        formats.command(
            tr!("files.convert.strip"),
            MaterialSymbol::LOCATION_OFF,
            |window: &mut WindowState| window.convert_to(Target::StripMetadata),
        );
    }
    formats.rule();
    formats.command(
        tr!("files.convert.more"),
        MaterialSymbol::TUNE,
        WindowState::show_convert,
    );
    formats
}
