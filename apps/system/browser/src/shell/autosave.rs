use std::time::Duration;

use browser_store::Timestamp;
use rikka_app::prelude::*;
use rikka_web::Session;

use crate::shell::Shell;
use browser_model::TabId;

const SAVE: &str = "rikka-browser-save";
const WATCH: &str = "rikka-browser-watch";
const SAVE_DELAY: Duration = Duration::from_secs(2);
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60);

impl Shell {
    pub(crate) fn schedule_save(&mut self) {
        self.browser.mark_session_dirty();
        after_keyed(SAVE, SAVE_DELAY, |shell: &mut Shell| {
            shell.browser.save_session();
        });
    }

    pub(crate) fn start_autosave(&mut self) {
        self.archive_idle();
        every_keyed(WATCH, AUTOSAVE_INTERVAL, |shell: &mut Shell| {
            shell.autosave()
        });
    }

    fn autosave(&mut self) {
        if let Some((tab, _)) = &self.shown {
            self.capture_state(*tab);
        }
        self.archive_idle();
        self.browser.save_session();
    }

    pub(crate) fn archive_idle(&mut self) {
        let Some(idle) = self.browser.preferences().archive_after().idle() else {
            return;
        };
        let before = Timestamp::now().saturating_sub(idle);
        let closing = self.browser.edit().archive_idle(self.id, before);
        if closing.is_empty() {
            return;
        }
        self.save_sessions(closing);
        self.changed();
    }

    pub(crate) fn capture_state(&mut self, tab: TabId) {
        let Some((view, url)) = self.live_view(tab) else {
            return;
        };
        if !self.model().writes(self.id) {
            return;
        }
        self.capture_tab_state(tab, url, &view);
    }

    fn capture_tab_state(&mut self, tab: TabId, url: String, view: &Session) {
        let state = view.state();
        view.flush_state();
        if state.is_empty() {
            return;
        }
        let changed = self
            .browser
            .edit()
            .tab_mut(tab)
            .is_some_and(|tab_entry| tab_entry.capture(url, state.to_bytes().into()));
        if changed {
            self.browser.mark_session_dirty();
        }
    }

    pub(crate) fn gather(&mut self, tabs: Vec<TabId>, then: fn(&mut Shell)) {
        for tab in tabs {
            if let Some((view, url)) = self.live_view(tab) {
                self.capture_tab_state(tab, url, &view);
            }
        }
        then(self);
    }

    fn live_view(&self, tab: TabId) -> Option<(Session, String)> {
        let model = self.model();
        let tab_entry = model.tab(tab)?;
        tab_entry.persistent_profile()?;
        let view = tab_entry.page().view()?.clone();
        Some((view, (*tab_entry.meta.url).clone()))
    }
}
