use rikka_web::{NavigationButton, Session};

use crate::page::Page;
use crate::shell::Shell;
use browser_model::{Placement, TabId, zoom_step};
use browser_pages::Internal;

const DEFAULT_ZOOM: f64 = 1.0;
const CHROME_DEFAULT_ZOOM: f64 = 0.0;
const VIEW_SOURCE: &str = "view-source:";

impl Shell {
    pub(crate) fn leave_address(&mut self) {
        self.address.end();
    }

    pub(crate) fn go(&mut self, destination: String) {
        let active = self.active_tab();
        let tab = match active {
            Some(tab) => tab,
            None => {
                let opened = self
                    .browser
                    .edit()
                    .open_tab(self.id, None, Placement::Top, true);
                let Some(opened) = opened else {
                    return;
                };
                opened
            }
        };
        self.go_in(tab, destination);
    }

    pub(crate) fn go_in(&mut self, tab: TabId, destination: String) {
        self.navigate(tab, destination);
        self.changed();
    }

    fn navigate(&mut self, tab: TabId, destination: String) {
        let mut model = self.browser.edit();
        let Some(tab_entry) = model.tab_mut(tab) else {
            return;
        };
        if let Some(internal) = Internal::parse(&destination) {
            tab_entry.internal.set(Some(internal));
            return;
        }
        tab_entry.internal.set(None);
        match tab_entry.page().view() {
            Some(view) => view.load(destination),
            None => {
                tab_entry.meta.url.set(destination);
                tab_entry.set_page(Page::Unloaded);
            }
        }
    }

    pub(crate) fn open_internal(&mut self, internal: Internal) {
        self.browser
            .edit()
            .open_tab(self.id, Some(internal.address()), Placement::Top, true);
        self.address.end();
        self.changed();
    }

    pub(crate) fn open_settings(&mut self) {
        self.open_internal(Internal::Settings(None));
    }

    pub(crate) fn back(&mut self) {
        let mut model = self.browser.edit();
        let Some(tab_entry) = model.active_mut(self.id) else {
            return;
        };
        if tab_entry.internal.is_some()
            && (tab_entry.page().live().is_some() || !tab_entry.meta.url.is_empty())
        {
            tab_entry.internal.set(None);
            drop(model);
            self.changed();
            return;
        }
        if let Some(view) = tab_entry.page().view() {
            view.back();
        }
    }

    pub(crate) fn forward(&mut self) {
        if let Some(view) = self.active_view() {
            view.forward();
        }
    }

    pub(crate) fn navigation_button(&mut self, button: NavigationButton) {
        match button {
            NavigationButton::Back => self.back(),
            NavigationButton::Forward => self.forward(),
        }
    }

    pub(crate) fn reload(&mut self, bypassing_cache: bool) {
        match (self.active_view(), bypassing_cache) {
            (Some(view), true) => view.reload_bypassing_cache(),
            (Some(view), false) => view.reload(),
            (None, _) => {}
        }
    }

    pub(crate) fn stop(&mut self) {
        if let Some(view) = self.active_view() {
            view.stop();
        }
    }

    pub(crate) fn zoom(&mut self, by: i32) {
        let Some(view) = self.active_view() else {
            return;
        };
        match by {
            0 => view.set_zoom(CHROME_DEFAULT_ZOOM),
            by => view.set_zoom(zoom_step(view.page().zoom, by, DEFAULT_ZOOM)),
        }
    }

    pub(crate) fn view_active_source(&mut self) {
        if let Some(tab) = self.active_tab() {
            self.view_source(tab);
        }
    }

    pub(crate) fn view_source(&mut self, tab: TabId) {
        let Some(address) = self
            .model()
            .tab(tab)
            .filter(|tab_entry| tab_entry.internal.is_none())
            .map(|tab_entry| (*tab_entry.meta.url).clone())
            .filter(|address| !address.is_empty() && !address.starts_with(VIEW_SOURCE))
        else {
            return;
        };
        let source = format!("{VIEW_SOURCE}{address}");
        match self.little() {
            true => self.open_in_browser_window(source),
            false => self.open_page(source),
        }
    }

    pub(crate) fn print_page(&mut self) {
        if let Some(view) = self.active_view() {
            view.print();
        }
    }

    pub(crate) fn active_view(&self) -> Option<Session> {
        self.model()
            .active(self.id)
            .and_then(|tab_entry| tab_entry.page().view().cloned())
    }
}
