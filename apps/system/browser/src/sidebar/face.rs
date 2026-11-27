use browser_model::Favicon;
use rikka_app::prelude::*;

use crate::address;
use crate::internal::route::Presented;
use crate::model::{Model, Tab};
use browser_model::{Saved, Slot, Sound, TabId, WindowId};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Face {
    pub(crate) title: String,
    pub(crate) favicon: Option<Favicon>,
    pub(crate) symbol: MaterialSymbol,
    pub(crate) tab: Option<TabId>,
    pub(crate) lit: bool,
    pub(crate) loading: bool,
    pub(crate) sound: Sound,
    pub(crate) discarded: bool,
}

impl Face {
    pub(crate) fn for_open_tab(model: &Model, window: WindowId, tab: TabId) -> Option<Face> {
        let active = model.window(window)?.active();
        model
            .tab(tab)
            .map(|tab_entry| Face::from_tab(tab_entry, active == Some(tab)))
    }

    pub(crate) fn for_slot(model: &Model, window: WindowId, slot: Slot) -> Option<Face> {
        let opened = model.window(window)?.tab_in_slot(slot);
        if let Some(face) = opened.and_then(|tab| Face::for_open_tab(model, window, tab)) {
            return Some(face);
        }
        let saved = match slot {
            Slot::Pinned(pin) => &model.pinned.pin(pin)?.1.saved,
            Slot::Favorite(favorite) => &model.favorites.find(favorite)?.1.saved,
        };
        Some(Face::from_saved(saved))
    }

    pub(crate) fn from_tab(tab: &Tab, lit: bool) -> Face {
        let (title, symbol) = match &*tab.internal {
            Some(internal) => (internal.title().to_string(), internal.symbol()),
            None if tab.is_blank() => (tr!("browser.tab.new").to_string(), MaterialSymbol::TAB),
            None => (
                titled(&tab.meta.title, &tab.meta.url),
                MaterialSymbol::PUBLIC,
            ),
        };
        Face {
            title,
            favicon: match tab.internal.is_some() {
                true => None,
                false => (*tab.meta.favicon).clone(),
            },
            symbol,
            tab: Some(tab.id),
            lit,
            loading: *tab.meta.loading,
            sound: *tab.meta.sound,
            discarded: tab.internal.is_none()
                && !tab.is_blank()
                && tab.page().view().is_none_or(|view| view.page().discarded),
        }
    }

    pub(crate) fn from_saved(saved: &Saved) -> Face {
        Face {
            title: titled(&saved.title, &saved.url),
            favicon: saved.favicon.clone(),
            symbol: MaterialSymbol::PUBLIC,
            tab: None,
            lit: false,
            loading: false,
            sound: Sound::Silent,
            discarded: false,
        }
    }
}

pub(crate) fn titled(title: &str, url: &str) -> String {
    let title = title.trim();
    if !title.is_empty() {
        return title.to_owned();
    }
    let shown = address::display_address(url);
    match shown.host.is_empty() {
        true => shown.rest,
        false => shown.host,
    }
}
