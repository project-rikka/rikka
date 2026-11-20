mod archives;
mod clipboard;
mod commands;
mod conversions;
mod dialog;
mod dropping;
mod items;
mod jobs;
pub(crate) mod keys;
mod menus;
mod places;
mod properties;
mod search;
mod strip;
mod undo;
mod view;
mod wallpaper;

use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;

use files_core::{Entry, Location, SortKey, ViewMode};
use files_vfs::Change;
use rikka_app::prelude::*;
use rikka_format::format_bytes;

use crate::devices::Devices;
use crate::manager::Windows;
use crate::services::Services;
use crate::tab::{ChangeSet, Listed, Tab};
use crate::tabs::Tabs;
use crate::{frame, listing, wording};

use jobs::Jobs;
use menus::Menus;

pub(crate) use dialog::{Asking, Compressing, Converting, Dialog};
pub(crate) use view::{strip, view};

pub(crate) const TABS: &str = "rikka.files.tab";

#[derive(State)]
pub(crate) struct WindowState {
    pub(crate) services: Services,
    pub(crate) tabs: Tabs,
    #[state]
    pub(crate) jobs: Jobs,
    #[state]
    pub(crate) menus: Menus,
    #[state]
    devices: Devices,
    pub(crate) dialog: frame::Shown<Dialog>,
    seeking: bool,
    sender: Sender<WindowState>,
    windows: Windows,
}

impl WindowState {
    pub(crate) fn new(
        sender: Sender<WindowState>,
        windows: Windows,
        start: Location,
        selected: Vec<Location>,
    ) -> Self {
        let mut tabs = Tabs::new(start);
        tabs.active_mut().select_on_arrival(selected);
        Self::with_tabs(sender, windows, tabs)
    }

    pub(crate) fn with_tab(sender: Sender<WindowState>, windows: Windows, tab: Tab) -> Self {
        Self::with_tabs(sender, windows, Tabs::with_tab(tab))
    }

    fn with_tabs(sender: Sender<WindowState>, windows: Windows, tabs: Tabs) -> Self {
        let services = Services::attach(
            &sender,
            WindowState::thumbnails_arrived,
            Devices::look,
            || Arc::new(rikka_mime::Database::load()),
        );
        let devices = Devices::new(&services, WindowState::device_command_finished).automounting();
        services.preload_applications();
        Self {
            services,
            tabs,
            jobs: Jobs::new(WindowState::job_finished),
            menus: Menus::default(),
            devices,
            dialog: frame::Shown::default(),
            seeking: false,
            sender,
            windows,
        }
    }

    pub(crate) fn start(&mut self) {
        let here = self.tabs.active_id();
        self.enter_folder(here);
        self.devices.look();
    }

    fn read(&self, id: u64) {
        let Some(tab) = self.tabs.by_id(id) else {
            return;
        };
        listing::read(
            &self.services,
            tab.at(),
            tab.shows_hidden(),
            move |window: &mut WindowState, listed: Listed| window.listed(id, listed),
        );
    }

    fn go(&mut self, id: u64, to: Location) {
        let Some(tab) = self.tabs.by_id_mut(id) else {
            return;
        };
        if tab.go(to) {
            self.enter_folder(id);
        } else {
            self.refresh(id);
        }
    }

    pub(crate) fn navigate_active(&mut self, to: Location) {
        let here = self.tabs.active_id();
        self.go(here, to);
    }

    fn enter_folder(&mut self, id: u64) {
        let Some(tab) = self.tabs.by_id_mut(id) else {
            return;
        };
        listing::enter_folder(&self.services, tab, &self.sender, move |window, change| {
            window.changed(id, change);
        });
        if self.tabs.active_id() == id {
            self.services.reload_places();
        }
        self.read(id);
    }

    fn refresh(&mut self, id: u64) {
        let Some(tab) = self.tabs.by_id_mut(id) else {
            return;
        };
        tab.start_loading();
        self.read(id);
    }

    pub(crate) fn refresh_active(&mut self) {
        self.refresh(self.tabs.active_id());
    }

    fn changed(&mut self, id: u64, change: Change) {
        let Some(tab) = self.tabs.by_id_mut(id) else {
            return;
        };
        if tab.record_change(change) {
            after(listing::CHANGE_DEBOUNCE, move |window: &mut WindowState| {
                window.settle(id);
            });
        }
    }

    fn settle(&mut self, id: u64) {
        let Some(tab) = self.tabs.by_id_mut(id) else {
            return;
        };
        match tab.settle() {
            ChangeSet::Listing => self.refresh(id),
            ChangeSet::Entries(at) if at.is_empty() => {}
            ChangeSet::Entries(at) => listing::stat_entries(
                &self.services,
                at,
                move |window: &mut WindowState, fresh: Vec<Entry>| window.update_entries(id, fresh),
            ),
        }
    }

    fn update_entries(&mut self, id: u64, fresh: Vec<Entry>) {
        let Some(tab) = self.tabs.by_id_mut(id) else {
            return;
        };
        if tab.update_entries(fresh, &self.services) && self.tabs.active_id() == id {
            self.thumbnails();
        }
    }

    fn rebuild(&mut self, id: u64) {
        if let Some(tab) = self.tabs.by_id_mut(id) {
            tab.rebuild(&self.services);
        }
        if self.tabs.active_id() == id {
            self.thumbnails();
        }
    }

    pub(crate) fn back(&mut self) {
        if self.tabs.active_mut().back() {
            self.enter_folder(self.tabs.active_id());
        }
    }

    pub(crate) fn forward(&mut self) {
        if self.tabs.active_mut().forward() {
            self.enter_folder(self.tabs.active_id());
        }
    }

    pub(crate) fn up(&mut self) {
        if let Some(at) = self.services.vfs.parent_of(self.tabs.active().at()) {
            self.navigate_active(at);
        }
    }

    pub(crate) fn home(&mut self) {
        self.navigate_active(listing::home());
    }

    pub(crate) fn path(&mut self) {
        let text = self.tabs.active_mut().typed_path();
        match listing::typed(&self.services, &text) {
            Ok(at) => self.navigate_active(at),
            Err(reason) => self.fail(reason),
        }
    }

    fn listed(&mut self, id: u64, listed: Listed) {
        if self.tabs.is_empty() {
            return;
        }
        let here = self.tabs.active_id();
        let Some(tab) = self.tabs.by_id_mut(id) else {
            return;
        };
        if listed.at != *tab.at() {
            return;
        }
        let reselected = tab.listed(listed, &self.services);
        if id == here {
            if reselected {
                self.tabs.active_mut().recheck_selection(&self.services.vfs);
            }
            self.thumbnails();
        }
    }

    fn thumbnails(&self) -> bool {
        self.tabs.active().refresh_thumbnails(&self.services)
    }

    fn thumbnails_arrived(&mut self, arrived: Vec<Location>) {
        if self.tabs.is_empty() {
            skip_view();
            return;
        }
        self.tabs
            .active()
            .thumbnails_arrived(&self.services, arrived);
    }

    pub(crate) fn visible(&mut self, range: Range<usize>) {
        self.tabs.active_mut().show_range(range, &self.services);
    }

    pub(crate) fn open(&mut self, index: usize) {
        let Some(row) = self.tabs.active().rows().get(index) else {
            return;
        };
        if row.is_directory() {
            let at = row.at.clone();
            self.navigate_active(at);
            return;
        }
        let Some(entry) = self.tabs.active().entry(index) else {
            return;
        };
        if self.services.vfs.browses(&entry.location) {
            let at = entry.location.clone();
            self.tabs.active_mut().start_loading();
            self.browse(at, None);
            return;
        }
        if files_open::is_program(entry) {
            let program = entry.location.clone();
            self.dialog.show(Dialog::ConfirmRun { program });
            return;
        }
        if let Err(error) = self
            .services
            .applications()
            .open(entry, &self.services.mime)
        {
            self.fail(wording::launch(&error));
        }
    }

    pub(crate) fn open_beside(&mut self, index: usize) {
        let Some(row) = self.tabs.active().rows().get(index) else {
            return;
        };
        if !row.is_directory() {
            self.open(index);
            return;
        }
        let at = row.at.clone();
        self.open_tab(at, false);
    }

    pub(crate) fn select(&mut self, chosen: HashSet<usize>, focus: Option<usize>) {
        let tab = self.tabs.active_mut();
        tab.choose(chosen, focus);
        tab.recheck_selection(&self.services.vfs);
    }

    pub(crate) fn sort_listing(&mut self, key: SortKey) {
        self.tabs.active_mut().sort_by(key);
        self.rebuild(self.tabs.active_id());
    }

    pub(crate) fn view_as(&mut self, mode: ViewMode) {
        self.tabs.active_mut().view(mode);
        self.thumbnails();
    }

    pub(crate) fn toggle_hidden(&mut self) {
        self.tabs.active_mut().toggle_hidden();
        self.refresh_active();
    }

    pub(crate) fn select_all(&mut self) {
        let tab = self.tabs.active();
        self.select(tab.all_rows(), tab.focus());
    }

    pub(crate) fn select_none(&mut self) {
        self.select(HashSet::new(), None);
    }

    pub(crate) fn seek(&mut self) {
        self.menus.close();
        self.tabs.active().finding().text().select_all();
        self.seeking = true;
    }

    pub(crate) fn sought(&mut self) {
        if !self.seeking {
            skip_view();
            return;
        }
        self.seeking = false;
    }

    pub(crate) fn seeking(&self) -> bool {
        self.seeking
    }

    pub(crate) fn requested(&mut self, request: files_ipc::Request) {
        if self.tabs.is_empty() {
            return;
        }
        let Some((at, selected)) = request.reveal.destination() else {
            return;
        };
        match self.tabs.showing(&at) {
            Some(index) => {
                self.tabs.select(index);
                self.refresh_active();
            }
            None => self.open_tab(at, true),
        }
        self.tabs.active_mut().select_on_arrival(selected);
        if let Some(token) = request.activation_token {
            request_window(WindowRequest::Activate(token));
        }
    }

    pub(crate) fn fail(&mut self, reason: String) {
        self.tabs.active_mut().fail(reason);
    }

    pub(crate) fn dismiss_failure(&mut self) {
        self.tabs.active_mut().dismiss_failure();
    }

    pub(crate) fn summary(&self) -> String {
        let tab = self.tabs.active();
        if tab.is_loading() {
            return tr!("files.status.loading").to_string();
        }
        if tab.finding().is_running() {
            return tr!("files.status.searching", count = tab.finding().waiting()).to_string();
        }
        let total = tab.rows().len();
        let count = tab.chosen_count();
        if count == 0 {
            return tr!("files.status.items", count = total).to_string();
        }
        let selected: u64 = tab
            .selected_entries()
            .iter()
            .filter(|entry| !entry.is_directory())
            .map(|entry| entry.size)
            .sum();
        if selected == 0 {
            return tr!("files.status.selected", count = count, total = total).to_string();
        }
        tr!(
            "files.status.selected-size",
            count = count,
            total = total,
            size = &format_bytes(selected),
        )
        .to_string()
    }
}

impl std::fmt::Debug for WindowState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowState")
            .field("tabs", &self.tabs.len())
            .finish_non_exhaustive()
    }
}
