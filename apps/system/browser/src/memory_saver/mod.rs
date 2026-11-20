mod settings;

pub(crate) use settings::view as settings_view;

use std::time::Duration;

use browser_pages::{TabActivity, discardable_tabs, is_exempt, is_pressed, web_host};
use browser_store::Timestamp;
use rikka_app::prelude::*;
use rikka_web::Session;

use crate::page::Page;
use crate::shell::Shell;
use browser_model::{Capturing, Sound, TabId};

const PRESSURE: &str = "/proc/pressure/memory";

const WATCH: &str = "rikka-browser-memory";
const EVERY: Duration = Duration::from_secs(60);
const PRESSED_IDLE: Duration = Duration::from_secs(2 * 60);

impl Shell {
    pub(crate) fn watch_memory(&mut self) {
        every_keyed(WATCH, EVERY, |shell: &mut Shell| shell.save_memory());
    }

    fn save_memory(&mut self) {
        skip_view();
        if self.browser().preferences().memory_saver().idle().is_none() {
            return;
        }
        spawn_blocking(
            || std::fs::read_to_string(PRESSURE).is_ok_and(|pressure| is_pressed(&pressure)),
            |shell: &mut Shell, pressed: bool| {
                shell.discard_idle(pressed);
            },
        );
    }

    pub(crate) fn discard_idle(&mut self, pressed: bool) {
        let Some(idle) = self.browser().preferences().memory_saver().idle() else {
            return;
        };
        let idle = match pressed {
            true => idle.min(PRESSED_IDLE),
            false => idle,
        };
        for tab in discardable_tabs(&self.activities(), Timestamp::now(), idle) {
            self.discard(tab);
        }
    }

    pub(crate) fn discard(&mut self, tab: TabId) {
        let Some(view) = self.discardable_view(tab) else {
            return;
        };
        let state = view.state();
        if state.is_empty() {
            return;
        }
        view.set_visible(false);
        if let Some(tab_entry) = self.browser().edit().tab_mut(tab) {
            tab_entry.set_page(Page::Discarded(state.to_bytes().into()));
        }
        self.changed();
    }

    fn discardable_view(&self, tab: TabId) -> Option<Session> {
        let activity = self.activity(tab)?;
        let idle = Duration::ZERO;
        if !activity.is_discardable(Timestamp::now(), idle) {
            return None;
        }
        self.view_of(tab)
    }

    fn activities(&self) -> Vec<TabActivity> {
        let tabs = self
            .model()
            .window(self.id())
            .map(|layout| layout.tabs())
            .unwrap_or_default();
        tabs.into_iter()
            .filter_map(|tab| self.activity(tab))
            .collect()
    }

    fn activity(&self, tab: TabId) -> Option<TabActivity> {
        let model = self.model();
        let layout = model.window(self.id())?;
        let tab_entry = model.tab(tab)?;
        let live = tab_entry.page().live();
        let seen = layout.active() == Some(tab)
            || model
                .shown_split(self.id())
                .is_some_and(|group| group.holds(tab))
            || self.peek.holds(tab);
        let never = self.browser().preferences().never_discarded();
        Some(TabActivity {
            tab,
            live: live.is_some() && tab_entry.internal.is_none(),
            seen,
            last_active: tab_entry.last_active,
            playing: *tab_entry.meta.sound == Sound::Playing,
            capturing: *tab_entry.meta.capture != Capturing::default(),
            asking: self.awaits_user(tab),
            exempt: is_exempt(web_host(&tab_entry.meta.url).as_deref(), &never),
        })
    }

    fn awaits_user(&self, tab: TabId) -> bool {
        let page_services = self.browser().page_services();
        let requests = page_services.requests();
        requests.get(tab).is_some_and(|tab_requests| {
            tab_requests.dialogs.current().is_some()
                || !tab_requests.prompts.is_empty()
                || tab_requests.signing.is_some()
                || tab_requests.picking_files
        })
    }
}

const ASLEEP: f32 = 0.55;

pub(crate) fn dimmed(icon: Element, discarded: bool) -> Element {
    match discarded {
        true => icon.opacity(ASLEEP),
        false => icon,
    }
}
