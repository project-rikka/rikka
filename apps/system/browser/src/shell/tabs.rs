use rikka_app::prelude::*;

use crate::shell::{Shell, WindowContent, window};
use browser_model::{Placement, Row, Slot, SpaceId, TabId, Target};

impl Shell {
    pub(crate) fn new_tab(&mut self) {
        self.browser
            .edit()
            .open_tab(self.id, None, Placement::Top, true);
        self.address.begin(String::new());
        self.changed();
    }

    pub(crate) fn select(&mut self, tab: TabId) {
        self.browser.edit().select(self.id, tab);
        self.address.end();
        self.changed();
    }

    pub(crate) fn select_slot(&mut self, slot: Slot) {
        self.browser.edit().select_slot(self.id, slot);
        self.address.end();
        self.changed();
    }

    pub(crate) fn close(&mut self, tab: TabId) {
        let successor = self.pane_successor(tab);
        let closing = self.browser.edit().close_tab(self.id, tab);
        if let Some(successor) = successor {
            self.browser.edit().select(self.id, successor);
        }
        self.close_views(closing);
        self.changed();
    }

    pub(crate) fn close_active(&mut self) {
        let active = self.active_tab();
        if let Some(tab) = active {
            self.close(tab);
        }
    }

    pub(crate) fn close_below(&mut self, tab: TabId) {
        let closing = self.browser.edit().close_below(self.id, tab);
        self.close_views(closing);
        self.changed();
    }

    pub(crate) fn clear_today(&mut self) {
        let closing = self.browser.edit().clear_today(self.id);
        self.close_views(closing);
        self.changed();
    }

    pub(crate) fn duplicate(&mut self, tab: TabId) {
        self.browser.edit().duplicate(self.id, tab);
        self.changed();
    }

    pub(crate) fn toggle_pin(&mut self, tab: TabId) {
        self.browser.edit().toggle_pin(self.id, tab);
        self.changed();
    }

    pub(crate) fn add_favorite(&mut self, tab: TabId) {
        self.browser.edit().favorite(self.id, tab);
        self.changed();
    }

    pub(crate) fn move_to_space(&mut self, tab: TabId, space: SpaceId) {
        self.browser.edit().move_to_space(self.id, tab, space);
        self.changed();
    }

    pub(crate) fn dropped(&mut self, row: Row, target: Target) {
        self.browser.edit().drop_row(self.id, row, target);
        self.changed();
    }

    pub(crate) fn tear_off(&mut self, tab: TabId) {
        let Some(private) = self.model().window(self.id).map(|layout| layout.private) else {
            return;
        };
        let browser = self.browser.clone();
        open_window(window(&browser, private, WindowContent::Tab(tab)));
    }

    pub(crate) fn toggle_mute(&mut self, tab: TabId) {
        let Some(view) = self.view_of(tab) else {
            return;
        };
        view.set_muted(!view.page().muted);
        self.follow(tab, &view.page());
    }

    pub(crate) fn copy_link(&mut self, tab: TabId) {
        if let Some(address) = self.model().tab(tab).map(|tab_entry| tab_entry.address()) {
            rikka_ui::clipboard::set_text(&address);
        }
    }
}
