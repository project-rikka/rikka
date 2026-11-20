use browser_store::Store;
use rikka_app::prelude::*;

use crate::home::tile::{GLYPH, shelf, tile};
use crate::menus::{Menus, Subject};
use crate::shell::Shell;
use crate::sidebar::titled;
use crate::widgets::{favicon, stored_favicon};
use browser_model::{Favorite, TabId};

pub(crate) fn view(shell: &Shell, tab: TabId) -> Option<Element> {
    let store = shell.browser().store();
    let tiles: Vec<Element> = shell
        .model()
        .favorites_here(shell.id())
        .iter()
        .map(|favorite| favorite_tile(favorite, tab, &store))
        .collect();
    if tiles.is_empty() {
        return None;
    }
    Some(shelf(tr!("browser.home.favorites"), tiles))
}

fn favorite_tile(favorite: &Favorite, tab: TabId, store: &Store) -> Element {
    let id = favorite.id;
    let saved = &favorite.saved;
    let look = match &saved.favicon {
        Some(image) => favicon(Some(image), MaterialSymbol::PUBLIC, GLYPH),
        None => stored_favicon(Some(store), &saved.url, MaterialSymbol::PUBLIC, GLYPH),
    };
    tile(look, titled(&saved.title, &saved.url))
        .on_press(move |shell: &mut Shell| shell.open_favorite_from_home(tab, id))
        .aim(move |menus: &mut Menus| menus.aim(Subject::Favorite(id)))
        .build()
        .keyed(id)
}
