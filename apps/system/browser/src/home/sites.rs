use browser_store::Store;
use rikka_app::prelude::*;
use rikka_web::HistoryItem;

use crate::home::tile::{GLYPH, shelf, tile};
use crate::menus::{Entries, Menus, Subject};
use crate::omnibox::Opening;
use crate::shell::Shell;
use crate::sidebar::titled;
use crate::widgets::stored_favicon;
use browser_model::TabId;

const SHOWN: usize = 8;

pub(crate) fn view(shell: &Shell, tab: TabId) -> Option<Element> {
    let favorites: Vec<String> = shell
        .model()
        .favorites_here(shell.id())
        .iter()
        .map(|favorite| favorite.saved.url.clone())
        .collect();
    let hidden = shell.browser().preferences().hidden_sites();
    let store = shell.browser().store();
    let tiles: Vec<Element> = shell
        .pages()
        .home
        .sites()
        .iter()
        .filter(|site| !favorites.contains(&site.url) && !hidden.contains(&site.url))
        .take(SHOWN)
        .map(|site| site_tile(site, tab, &store))
        .collect();
    if tiles.is_empty() {
        return None;
    }
    Some(shelf(tr!("browser.home.top-sites"), tiles))
}

fn site_tile(site: &HistoryItem, tab: TabId, store: &Store) -> Element {
    let id = site.id.clone();
    let (opened, aside) = (site.url.clone(), site.url.clone());
    tile(
        stored_favicon(Some(store), &site.url, MaterialSymbol::PUBLIC, GLYPH),
        titled(&site.title, &site.url),
    )
    .on_press(move |shell: &mut Shell| shell.open_from_home(tab, opened.clone()))
    .middle(move |shell: &mut Shell| shell.open_aside(aside.clone()))
    .aim(move |menus: &mut Menus| menus.aim(Subject::TopSite(id.clone())))
    .build()
    .keyed(site.id.clone())
}

pub(crate) fn menu(shell: &Shell, site: &str) -> Entries {
    let mut entries = Entries::new();
    let Some(site) = shell.pages().home.site(site) else {
        return entries;
    };
    let (aside, windowed, copied, hidden) = (
        site.url.clone(),
        site.url.clone(),
        site.url.clone(),
        site.url.clone(),
    );
    entries.leaf(
        menu_item(tr!("browser.home.open-in-tab"))
            .icon(MaterialSymbol::TAB)
            .on_press(move |shell: &mut Shell| shell.open_aside(aside.clone())),
    );
    entries.leaf(
        menu_item(tr!("browser.home.open-in-window"))
            .icon(MaterialSymbol::OPEN_IN_NEW)
            .on_press(move |shell: &mut Shell| {
                shell.open_address(windowed.clone(), Opening::NewWindow)
            }),
    );
    entries.leaf(
        menu_item(tr!("browser.menu.copy-link"))
            .icon(MaterialSymbol::LINK)
            .on_press(move |_: &mut Shell| rikka_ui::clipboard::set_text(&copied)),
    );
    entries.separator();
    entries.leaf(
        menu_item(tr!("browser.home.hide"))
            .icon(MaterialSymbol::VISIBILITY_OFF)
            .on_press(move |shell: &mut Shell| shell.hide_top_site(&hidden)),
    );
    entries
}
