use browser_pages::Cause;
use rikka_app::prelude::*;
use rikka_types::Choice;

use crate::command::Command;
use crate::internal::Span;
use crate::keys;
use crate::model::Model;
use crate::shell::Shell;
use crate::sidebar::Renaming;
use browser_model::{
    EMOJI, FavoriteId, FolderId, Home, PinnedTabId, ProfileKey, Slot, Sound, SpaceIcon, SpaceId,
    Symbol, TabId, WindowId,
};
use browser_pages::Internal;

pub(crate) struct Entries {
    items: Vec<MenuItem>,
    branches: Vec<bool>,
}

impl Entries {
    pub(crate) fn new() -> Entries {
        Entries {
            items: Vec::new(),
            branches: Vec::new(),
        }
    }

    pub(crate) fn leaf(&mut self, item: MenuItem) {
        self.items.push(item);
        self.branches.push(false);
    }

    pub(crate) fn branch(&mut self, item: MenuItem) {
        self.items.push(item);
        self.branches.push(true);
    }

    pub(crate) fn separator(&mut self) {
        if !self.items.is_empty() {
            self.leaf(menu_separator());
        }
    }

    pub(crate) fn finish(self) -> (Vec<MenuItem>, Vec<bool>) {
        (self.items, self.branches)
    }
}

pub(crate) fn tab(model: &Model, window: WindowId, tab: TabId) -> Entries {
    let mut entries = Entries::new();
    let Some(tab_entry) = model.tab(tab) else {
        return entries;
    };
    let persistent = tab_entry.profile != ProfileKey::Private;
    match tab_entry.home {
        Home::Today(_) if persistent => {
            entries.leaf(
                menu_item(tr!("browser.menu.pin"))
                    .icon(MaterialSymbol::KEEP)
                    .shortcut(keys::shortcut(Command::TogglePin))
                    .on_press(move |shell: &mut Shell| shell.toggle_pin(tab)),
            );
            entries.leaf(
                menu_item(tr!("browser.menu.favorite"))
                    .icon(MaterialSymbol::STAR)
                    .on_press(move |shell: &mut Shell| shell.add_favorite(tab)),
            );
        }
        Home::Pinned(_) => {
            entries.leaf(
                menu_item(tr!("browser.menu.unpin"))
                    .icon(MaterialSymbol::KEEP_OFF)
                    .shortcut(keys::shortcut(Command::TogglePin))
                    .on_press(move |shell: &mut Shell| shell.toggle_pin(tab)),
            );
        }
        Home::Today(_) | Home::Favorite(_) => {}
    }
    page_entries(&mut entries, model, tab);
    if let Home::Today(Some(current)) = tab_entry.home {
        let others: Vec<MenuItem> = model
            .spaces
            .list()
            .iter()
            .filter(|space| space.id != current)
            .map(|space| {
                let id = space.id;
                menu_item(space_name(&space.name))
                    .on_press(move |shell: &mut Shell| shell.move_to_space(tab, id))
            })
            .collect();
        if !others.is_empty() {
            entries.branch(
                menu_item(tr!("browser.menu.move-to-space"))
                    .icon(MaterialSymbol::DRIVE_FILE_MOVE)
                    .items(others),
            );
        }
    }
    crate::split::tab_entries(&mut entries, model, window, tab);
    entries.separator();
    entries.leaf(
        menu_item(tr!("browser.menu.close"))
            .icon(MaterialSymbol::CLOSE)
            .shortcut(keys::shortcut(Command::CloseTab))
            .on_press(move |shell: &mut Shell| shell.close(tab)),
    );
    let below = model
        .window(window)
        .map(|layout| layout.today_here())
        .and_then(|today| {
            today
                .iter()
                .position(|candidate| *candidate == tab)
                .map(|at| at + 1 < today.len())
        })
        .unwrap_or(false);
    if below {
        entries.leaf(
            menu_item(tr!("browser.menu.close-below"))
                .on_press(move |shell: &mut Shell| shell.close_below(tab)),
        );
    }
    entries
}

pub(crate) fn pin(model: &Model, window: WindowId, pin: PinnedTabId) -> Entries {
    let opened = model
        .window(window)
        .and_then(|layout| layout.tab_in_slot(Slot::Pinned(pin)));
    let mut entries = match opened {
        Some(tab) => self::tab(model, window, tab),
        None => Entries::new(),
    };
    let Some((_, pinned_tab)) = model.pinned.pin(pin) else {
        return entries;
    };
    if let Some(tab) = opened
        && model
            .tab(tab)
            .is_some_and(|tab| tab.address() != pinned_tab.saved.url)
    {
        let pinned = pinned_tab.saved.url.clone();
        entries.leaf(
            menu_item(tr!("browser.menu.back-to-pinned"))
                .icon(MaterialSymbol::RESTART_ALT)
                .on_press(move |shell: &mut Shell| {
                    shell.select(tab);
                    shell.go(pinned.clone(), Cause::Generated);
                }),
        );
    }
    if opened.is_none() {
        let address = pinned_tab.saved.url.clone();
        entries.leaf(
            menu_item(tr!("browser.menu.unpin"))
                .icon(MaterialSymbol::KEEP_OFF)
                .on_press(move |shell: &mut Shell| shell.unpin(pin)),
        );
        entries.leaf(
            menu_item(tr!("browser.menu.copy-link"))
                .icon(MaterialSymbol::LINK)
                .on_press(move |_: &mut Shell| rikka_ui::clipboard::set_text(&address)),
        );
    }
    entries
}

pub(crate) fn folder(model: &Model, folder: FolderId) -> Entries {
    let mut entries = Entries::new();
    let Some(folder_entry) = model.pinned.folder(folder) else {
        return entries;
    };
    let name = folder_entry.name.clone();
    entries.leaf(
        menu_item(tr!("browser.menu.rename"))
            .icon(MaterialSymbol::EDIT)
            .on_press(move |shell: &mut Shell| shell.rename(Renaming::Folder(folder), &name)),
    );
    entries.leaf(
        menu_item(tr!("browser.menu.new-folder"))
            .icon(MaterialSymbol::CREATE_NEW_FOLDER)
            .on_press(move |shell: &mut Shell| shell.new_folder(Some(folder))),
    );
    entries.separator();
    entries.leaf(
        menu_item(tr!("browser.menu.delete-folder"))
            .icon(MaterialSymbol::DELETE)
            .on_press(move |shell: &mut Shell| shell.delete_folder(folder)),
    );
    entries
}

pub(crate) fn favorite(model: &Model, window: WindowId, favorite: FavoriteId) -> Entries {
    let mut entries = Entries::new();
    let Some((_, favorite_entry)) = model.favorites.find(favorite) else {
        return entries;
    };
    let opened = model
        .window(window)
        .and_then(|layout| layout.tab_in_slot(Slot::Favorite(favorite)));
    if let Some(tab) = opened {
        page_entries(&mut entries, model, tab);
    } else {
        let address = favorite_entry.saved.url.clone();
        entries.leaf(
            menu_item(tr!("browser.menu.copy-link"))
                .icon(MaterialSymbol::LINK)
                .on_press(move |_: &mut Shell| rikka_ui::clipboard::set_text(&address)),
        );
    }
    entries.separator();
    entries.leaf(
        menu_item(tr!("browser.menu.unfavorite"))
            .icon(MaterialSymbol::BOOKMARK_REMOVE)
            .on_press(move |shell: &mut Shell| shell.unfavorite(favorite)),
    );
    if let Some(tab) = opened {
        entries.leaf(
            menu_item(tr!("browser.menu.close"))
                .icon(MaterialSymbol::CLOSE)
                .on_press(move |shell: &mut Shell| shell.close(tab)),
        );
    }
    entries
}

fn page_entries(entries: &mut Entries, model: &Model, tab: TabId) {
    let Some(tab_entry) = model.tab(tab) else {
        return;
    };
    entries.leaf(
        menu_item(tr!("browser.menu.duplicate"))
            .icon(MaterialSymbol::CONTENT_COPY)
            .on_press(move |shell: &mut Shell| shell.duplicate(tab)),
    );
    match *tab_entry.meta.sound {
        Sound::Playing => entries.leaf(
            menu_item(tr!("browser.menu.mute"))
                .icon(MaterialSymbol::VOLUME_OFF)
                .on_press(move |shell: &mut Shell| shell.toggle_mute(tab)),
        ),
        Sound::Muted => entries.leaf(
            menu_item(tr!("browser.menu.unmute"))
                .icon(MaterialSymbol::VOLUME_UP)
                .on_press(move |shell: &mut Shell| shell.toggle_mute(tab)),
        ),
        Sound::Silent => {}
    }
    entries.leaf(
        menu_item(tr!("browser.menu.copy-link"))
            .icon(MaterialSymbol::LINK)
            .on_press(move |shell: &mut Shell| shell.copy_link(tab)),
    );
}

pub(crate) fn space(model: &Model, space: SpaceId) -> Entries {
    let mut entries = Entries::new();
    let Some(space_entry) = model.spaces.get(space) else {
        return entries;
    };
    let name = space_name(&space_entry.name);
    entries.leaf(
        menu_item(tr!("browser.menu.rename"))
            .icon(MaterialSymbol::EDIT)
            .on_press(move |shell: &mut Shell| shell.rename(Renaming::Space(space), &name)),
    );
    let symbols = Symbol::ALL.iter().copied().map(|symbol| {
        let icon = SpaceIcon::Symbol(symbol);
        let chosen = space_entry.icon == icon;
        menu_item(String::new())
            .icon(symbol.glyph())
            .selected(chosen)
            .on_press(move |shell: &mut Shell| shell.set_space_icon(space, icon.clone()))
    });
    let emoji = EMOJI.into_iter().map(|emoji| {
        let icon = SpaceIcon::Emoji(emoji.to_owned());
        let chosen = space_entry.icon == icon;
        menu_item(emoji)
            .selected(chosen)
            .on_press(move |shell: &mut Shell| shell.set_space_icon(space, icon.clone()))
    });
    entries.branch(
        menu_item(tr!("browser.menu.icon"))
            .icon(MaterialSymbol::STAR)
            .items(symbols.chain([menu_separator()]).chain(emoji)),
    );
    let profiles = model.profiles.list().iter().map(|profile| {
        let id = profile.id;
        menu_item(profile_name(&profile.name))
            .selected(space_entry.profile == id)
            .on_press(move |shell: &mut Shell| shell.set_space_profile(space, id))
    });
    let fresh = menu_item(tr!("browser.menu.new-profile"))
        .icon(MaterialSymbol::ADD)
        .on_press(move |shell: &mut Shell| shell.new_profile_for(space));
    entries.branch(
        menu_item(tr!("browser.menu.profile"))
            .icon(MaterialSymbol::PERSON)
            .items(profiles.chain([menu_separator(), fresh])),
    );
    if model.spaces.list().len() > 1 {
        entries.separator();
        entries.leaf(
            menu_item(tr!("browser.menu.delete-space"))
                .icon(MaterialSymbol::DELETE)
                .on_press(move |shell: &mut Shell| shell.delete_space(space)),
        );
    }
    entries
}

pub(crate) fn main(private: bool) -> Entries {
    let mut entries = Entries::new();
    entries.leaf(
        menu_item(tr!("browser.menu.new-tab"))
            .icon(MaterialSymbol::TAB)
            .shortcut(keys::shortcut(Command::NewTab))
            .on_press(|shell: &mut Shell| shell.new_tab()),
    );
    entries.leaf(
        menu_item(tr!("browser.menu.new-window"))
            .icon(MaterialSymbol::NEW_WINDOW)
            .shortcut(keys::shortcut(Command::NewWindow))
            .on_press(|shell: &mut Shell| shell.command(Command::NewWindow)),
    );
    entries.leaf(
        menu_item(tr!("browser.menu.new-private-window"))
            .icon(MaterialSymbol::VISIBILITY_OFF)
            .shortcut(keys::shortcut(Command::PrivateWindow))
            .on_press(|shell: &mut Shell| shell.command(Command::PrivateWindow)),
    );
    entries.separator();
    if !private {
        entries.leaf(
            menu_item(tr!("browser.menu.history"))
                .icon(MaterialSymbol::HISTORY)
                .shortcut(keys::shortcut(Command::History))
                .on_press(|shell: &mut Shell| shell.command(Command::History)),
        );
        entries.leaf(
            menu_item(tr!("browser.downloads.title"))
                .icon(MaterialSymbol::DOWNLOAD)
                .shortcut(keys::shortcut(Command::Downloads))
                .on_press(|shell: &mut Shell| shell.command(Command::Downloads)),
        );
        entries.leaf(
            menu_item(tr!("browser.menu.archive"))
                .icon(MaterialSymbol::INVENTORY_2)
                .on_press(|shell: &mut Shell| shell.open_internal(Internal::Archive)),
        );
        entries.leaf(
            menu_item(tr!("browser.menu.clear-data"))
                .icon(MaterialSymbol::DELETE_SWEEP)
                .shortcut(keys::shortcut(Command::ClearData))
                .on_press(|shell: &mut Shell| shell.command(Command::ClearData)),
        );
        entries.separator();
    }
    entries.leaf(
        menu_item(tr!("browser.menu.zoom-in"))
            .icon(MaterialSymbol::ZOOM_IN)
            .shortcut(keys::shortcut(Command::ZoomIn))
            .on_press(|shell: &mut Shell| shell.zoom(1)),
    );
    entries.leaf(
        menu_item(tr!("browser.menu.zoom-out"))
            .icon(MaterialSymbol::ZOOM_OUT)
            .shortcut(keys::shortcut(Command::ZoomOut))
            .on_press(|shell: &mut Shell| shell.zoom(-1)),
    );
    entries.leaf(
        menu_item(tr!("browser.menu.zoom-reset"))
            .shortcut(keys::shortcut(Command::ZoomReset))
            .on_press(|shell: &mut Shell| shell.zoom(0)),
    );
    entries.leaf(
        menu_item(tr!("browser.fullscreen.menu"))
            .icon(MaterialSymbol::FULLSCREEN)
            .shortcut(keys::shortcut(Command::Fullscreen))
            .on_press(|shell: &mut Shell| shell.command(Command::Fullscreen)),
    );
    entries.separator();
    entries.leaf(
        menu_item(tr!("browser.find.menu"))
            .icon(MaterialSymbol::SEARCH)
            .shortcut(keys::shortcut(Command::Find))
            .on_press(|shell: &mut Shell| shell.command(Command::Find)),
    );
    entries.separator();
    entries.leaf(
        menu_item(tr!("browser.menu.save-pdf"))
            .icon(MaterialSymbol::PICTURE_AS_PDF)
            .shortcut(keys::shortcut(Command::SavePage))
            .on_press(|shell: &mut Shell| shell.save_page()),
    );
    entries.leaf(
        menu_item(tr!("browser.context.view-source"))
            .icon(MaterialSymbol::DATA_OBJECT)
            .shortcut(keys::shortcut(Command::ViewSource))
            .on_press(|shell: &mut Shell| shell.command(Command::ViewSource)),
    );
    entries.separator();
    entries.leaf(
        menu_item(tr!("browser.extensions.title"))
            .icon(MaterialSymbol::EXTENSION)
            .on_press(|shell: &mut Shell| shell.open_internal(browser_pages::Internal::Extensions)),
    );
    if !private {
        entries.leaf(
            menu_item(tr!("browser.menu.settings"))
                .icon(MaterialSymbol::SETTINGS)
                .shortcut(keys::shortcut(Command::Settings))
                .on_press(|shell: &mut Shell| shell.open_settings()),
        );
    }
    entries.leaf(
        menu_item(tr!("browser.menu.quit"))
            .icon(MaterialSymbol::LOGOUT)
            .on_press(|shell: &mut Shell| shell.quit()),
    );
    entries
}

pub(crate) fn history(model: &Model, window: WindowId, forward: bool) -> Entries {
    let mut entries = Entries::new();
    let Some(tab_entry) = model.active(window) else {
        return entries;
    };
    let can = match forward {
        true => *tab_entry.meta.forward,
        false => *tab_entry.meta.back,
    };
    let Some(view) = tab_entry.page().view().filter(|_| can) else {
        return entries;
    };
    let page = view.page();
    let Some(at) = page.history_index else {
        return entries;
    };
    let listed: Vec<(usize, &rikka_web::HistoryEntry)> = match forward {
        true => page.history.iter().enumerate().skip(at + 1).collect(),
        false => page.history.iter().enumerate().take(at).rev().collect(),
    };
    for (index, entry) in listed {
        let label = match entry.title.trim() {
            "" => entry.uri.clone(),
            title => title.to_owned(),
        };
        entries.leaf(
            menu_item(label).on_press(move |shell: &mut Shell| shell.go_to_history_index(index)),
        );
    }
    entries
}

pub(crate) fn clear_history() -> Entries {
    let mut entries = Entries::new();
    for span in Span::ALL {
        entries.leaf(
            menu_item(span.name()).on_press(move |shell: &mut Shell| shell.clear_history(span)),
        );
    }
    entries
}

pub(crate) fn space_name(name: &str) -> String {
    match name.is_empty() {
        true => tr!("browser.space.default").to_string(),
        false => name.to_owned(),
    }
}

pub(crate) fn profile_name(name: &str) -> String {
    match name.is_empty() {
        true => tr!("browser.profile.default").to_string(),
        false => name.to_owned(),
    }
}
