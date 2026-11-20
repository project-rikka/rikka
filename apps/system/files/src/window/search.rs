use std::sync::Arc;

use files_core::Entry;
use files_find::{Completion, Hit};
use rikka_app::prelude::*;
use rikka_file_operations::Store;
use rikka_ui::spawn_blocking;

use crate::window::WindowState;

impl WindowState {
    pub(crate) fn search(&mut self) {
        if self.tabs.active_mut().finding_mut().query_changed() {
            self.start_search();
        }
    }

    pub(super) fn toggle_search_contents(&mut self) {
        self.tabs.active_mut().finding_mut().toggle_contents();
        self.start_search();
    }

    fn start_search(&mut self) {
        let id = self.tabs.active_id();
        let sender = self.sender.clone();
        let over = self.sender.clone();
        let tab = self.tabs.active_mut();
        let (at, show_hidden) = (tab.at().clone(), tab.shows_hidden());
        let running = tab.finding_mut().start(
            &at,
            show_hidden,
            Arc::new(move |hit: Hit| {
                sender
                    .send(move |window: &mut WindowState| window.found(id, hit))
                    .ok();
            }),
            Arc::new(move |how: Completion| {
                over.send(move |window: &mut WindowState| window.search_over(id, how))
                    .ok();
            }),
        );
        if !running {
            self.refresh(id);
            return;
        }
        self.tabs.active_mut().clear();
        self.rebuild(id);
    }

    fn found(&mut self, id: u64, hit: Hit) {
        if self.tabs.is_empty() {
            return;
        }
        let enough = self
            .tabs
            .by_id_mut(id)
            .is_some_and(|tab| tab.finding_mut().found(hit.at.clone()));
        if enough {
            self.show_hits(id);
        }
    }

    fn search_over(&mut self, id: u64, how: Completion) {
        if self.tabs.is_empty() {
            return;
        }
        let Some(tab) = self.tabs.by_id_mut(id) else {
            return;
        };
        tab.finding_mut().finished();
        if how == Completion::Truncated {
            tab.fail(tr!("files.error.too-many").to_string());
        }
        self.show_hits(id);
    }

    fn show_hits(&mut self, id: u64) {
        let Some(tab) = self.tabs.by_id_mut(id) else {
            return;
        };
        let fresh = tab.finding_mut().take();
        if fresh.is_empty() {
            return;
        }
        let vfs = Arc::clone(&self.services.vfs);
        spawn_blocking(
            move || fresh.iter().filter_map(|at| vfs.stat(at).ok()).collect(),
            move |window: &mut WindowState, entries: Vec<Entry>| window.hits(id, entries),
        );
    }

    fn hits(&mut self, id: u64, entries: Vec<Entry>) {
        if self.tabs.is_empty() {
            return;
        }
        let Some(tab) = self.tabs.by_id_mut(id) else {
            return;
        };
        if !tab.finding().is_running() && entries.is_empty() {
            return;
        }
        tab.append(entries);
        self.rebuild(id);
    }
}
