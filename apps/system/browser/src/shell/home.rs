use browser_pages::Cause;
use browser_store::{ClosedTab, Restored};
use rikka_app::prelude::*;

use crate::shell::Shell;
use browser_model::{FavoriteId, Slot, TabId, take_closed_tab};

impl Shell {
    pub(super) fn prepare_home(&mut self, active: Option<TabId>, partners: &[TabId]) {
        let showing = {
            let model = self.model();
            active
                .into_iter()
                .chain(partners.iter().copied())
                .find(|tab| {
                    model
                        .tab(*tab)
                        .is_some_and(|tab_entry| tab_entry.shows_home())
                })
        };
        if showing == self.pages.home.shown_in() {
            return;
        }
        let Some(tab) = showing else {
            self.pages.home.close();
            return;
        };
        let (profile, spaces) = {
            let model = self.model();
            let profile = model.profile_here(self.id).persistent_id();
            let spaces = model
                .spaces
                .list()
                .iter()
                .filter(|space| Some(space.profile) == profile)
                .map(|space| space.id)
                .collect();
            (profile, spaces)
        };
        let store = self.browser.store();
        self.pages.home.open(tab, store, profile, spaces);
    }

    pub(crate) fn search_from_home(&mut self, tab: TabId) {
        if self.active_tab() != Some(tab) {
            self.browser.edit().select(self.id, tab);
        }
        self.address.begin(String::new());
        self.changed();
    }

    pub(crate) fn open_from_home(&mut self, tab: TabId, url: String) {
        self.address.end();
        self.go_in(tab, url, Cause::Link);
    }

    pub(crate) fn open_favorite_from_home(&mut self, tab: TabId, favorite: FavoriteId) {
        self.select_slot(Slot::Favorite(favorite));
        self.discard_blank(tab);
    }

    pub(crate) fn reopen_from_home(&mut self, tab: TabId, closed: ClosedTab) {
        let store = self.browser.store();
        spawn_blocking(
            move || take_closed_tab(&store, &closed),
            move |shell: &mut Shell, taken: browser_store::Result<Option<Restored>>| match taken {
                Ok(Some(restored)) => shell.reopened_in_place_of(tab, restored),
                Ok(None) => {}
                Err(error) => tracing::warn!(%error, "failed to reopen closed tab"),
            },
        );
    }

    pub(crate) fn reopened_in_place_of(&mut self, blank: TabId, restored: Restored) {
        self.reopened(restored);
        self.discard_blank(blank);
    }

    pub(crate) fn show_hidden_top_sites(&mut self) {
        if let Some(profile) = self.persistent_profile() {
            self.browser.store().show_hidden_top_sites(profile);
        }
    }

    fn discard_blank(&mut self, tab: TabId) {
        if !self
            .model()
            .tab(tab)
            .is_some_and(|tab_entry| tab_entry.is_blank())
        {
            return;
        }
        let closing = self.browser.edit().close_without_archive(self.id, tab);
        self.save_sessions(closing);
        self.changed();
    }
}
