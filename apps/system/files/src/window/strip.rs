use files_core::Location;
use rikka_app::prelude::*;

use crate::manager;
use crate::tab::Tab;
use crate::window::WindowState;

impl WindowState {
    pub(crate) fn open_tab(&mut self, at: Location, show: bool) {
        self.menus.close();
        let id = self.tabs.open(at, show);
        self.enter_folder(id);
    }

    pub(crate) fn new_tab(&mut self) {
        let at = self.tabs.active().at().clone();
        self.open_tab(at, true);
    }

    pub(crate) fn close_tab(&mut self, index: usize) {
        self.menus.close();
        if !self.tabs.close(index) {
            request_window(WindowRequest::Close);
            return;
        }
        self.thumbnails();
    }

    pub(crate) fn select_tab(&mut self, index: usize) {
        self.menus.close();
        self.tabs.select(index);
        self.thumbnails();
    }

    pub(super) fn next_tab(&mut self) {
        self.tabs.next();
        self.thumbnails();
    }

    pub(super) fn previous_tab(&mut self) {
        self.tabs.previous();
        self.thumbnails();
    }

    pub(super) fn nth_tab(&mut self, index: usize) {
        self.tabs.select_nth(index);
        self.thumbnails();
    }

    pub(crate) fn lift_tab(&mut self, index: usize) -> Option<Tab> {
        self.menus.close();
        self.tabs.take(index)
    }

    pub(crate) fn land_tab(&mut self, index: usize, tab: Tab) {
        let id = self.tabs.insert(index, tab);
        self.enter_folder(id);
    }

    pub(crate) fn tear_off_tab(&mut self, tab: Tab) {
        open_window(manager::window_with_tab(self.windows.clone(), tab));
    }

    pub(crate) fn new_window(&mut self) {
        let at = self.tabs.active().at().clone();
        open_window(manager::window_at(self.windows.clone(), at, Vec::new()));
    }

    pub(crate) fn close_active_tab(&mut self) {
        self.close_tab(self.tabs.active_index());
    }

    pub(crate) fn close_if_no_tabs(&mut self) {
        if self.tabs.is_empty() {
            request_window(WindowRequest::Close);
        }
    }
}
