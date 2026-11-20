use std::rc::Rc;

use browser_pages::Cause;
use rikka_app::prelude::*;
use rikka_appearance::Appearance;
use rikka_web::{CloseReason, Event, Hang, LoadError, RuntimeEvent};

use crate::interstitial::{Ended, document, problem_of};
use crate::page::Page;
use crate::requests::DropdownOwner;
use crate::shell::Shell;
use browser_model::Scope;
use browser_model::{ProfileKey, TabId};

const RUNTIME_FEED: &str = "rikka-browser-runtime-events";
const ALERT_FEED: &str = "rikka-browser-notification-events";

impl Shell {
    pub(crate) fn handle_request(&mut self, tab: TabId, event: Event) {
        match event {
            Event::Prompt(prompt) => self.prompted(tab, prompt),
            Event::PromptUpdated(prompt) => self.prompt_updated(tab, prompt),
            Event::PromptDismissed(id) => self.prompt_withdrawn(tab, &id),
            Event::Permission(request) => self.permission_asked(tab, request),
            Event::Media(request) => self.media_asked(tab, request),
            Event::ContextMenu(menu) => self.context_menu(tab, menu),
            Event::Dropdown(dropdown) => self.dropdown_opened(DropdownOwner::Tab(tab), dropdown),
            Event::DropdownUpdated(update) => {
                self.dropdown_updated(DropdownOwner::Tab(tab), update)
            }
            Event::DropdownHidden(id) => self.dropdown_hidden(DropdownOwner::Tab(tab), id),
            Event::Fullscreen(on) => self.page_fullscreen(tab, on),
            Event::Closed(CloseReason::EngineExited) => self.gone(tab, Ended::EngineExited),
            Event::ContentCrashed => self.gone(tab, Ended::Crashed),
            Event::ContentKilled => self.gone(tab, Ended::Killed),
            Event::Hang(hang) => self.hung(tab, hang),
            Event::LoadError(error) => self.load_failed(tab, error),
            Event::Download(request) => self.download_asked(tab, request),
            Event::Tooltip(tooltip) => self.page_tooltip(tab, tooltip),
            Event::FocusRequested => self.select(tab),
            Event::ExtensionAction(change) => self.tab_action_changed(tab, change),
            Event::ExtensionPopup(popup) => self.action_popup_opened(tab, popup),
            Event::ExtensionTabUpdate(request) => self.extension_updates_tab(tab, request),
            Event::ExtensionTabClose(request) => self.extension_closes_tab(tab, request),
            unanswered => drop(unanswered),
        }
    }

    pub(crate) fn scope_of(&self, tab: TabId) -> Option<Scope> {
        match self.model().tab(tab)?.profile {
            ProfileKey::Persistent(profile) => Some(Scope::Profile(profile)),
            ProfileKey::Private => Some(Scope::Window(self.id())),
        }
    }

    pub(crate) fn page_started(&mut self, tab: TabId, uri: &str) {
        let released = {
            let page_services = self.browser().page_services();
            let mut requests = page_services.requests_mut();
            let Some(tab_requests) = requests.edit_existing(tab) else {
                return;
            };
            tab_requests.failure.started(uri);
            let pointing = tab_requests.pointing.take();
            let choosing = tab_requests.choosing.take();
            let dropdown = std::mem::take(&mut tab_requests.dropdown);
            (
                std::mem::take(&mut tab_requests.prompts),
                pointing,
                choosing,
                dropdown,
            )
        };
        drop(released);
        let browser = Rc::clone(self.browser());
        let model = browser.model();
        let alive = |tab| model.tab(tab).is_some();
        self.reader.retain(alive);
        drop(model);
    }

    fn load_failed(&mut self, tab: TabId, error: LoadError) {
        let Some(problem) = problem_of(&error) else {
            error.show_nothing();
            return;
        };
        let dark = self.private() || Appearance::current().theme().is_dark();
        let page = document(&problem, dark);
        self.browser()
            .page_services()
            .requests_mut()
            .edit(tab)
            .failure
            .failed(error.uri.clone(), problem);
        error.show(page);
    }

    fn gone(&mut self, tab: TabId, ended: Ended) {
        let released = {
            let page_services = self.browser().page_services();
            let mut requests = page_services.requests_mut();
            let tab_requests = requests.edit(tab);
            tab_requests.failure.gone(ended);
            (
                std::mem::take(&mut tab_requests.prompts),
                tab_requests.signing.take(),
                tab_requests.pointing.take(),
                tab_requests.choosing.take(),
                std::mem::take(&mut tab_requests.dropdown),
            )
        };
        drop(released);
        self.page_left_fullscreen(tab);
    }

    fn hung(&mut self, tab: TabId, hang: Hang) {
        let page_services = self.browser().page_services();
        let mut requests = page_services.requests_mut();
        requests.edit(tab).failure.hang(hang);
    }

    pub(crate) fn retry(&mut self, tab: TabId, uri: String) {
        self.clear_failure(tab);
        let mut model = self.browser().edit();
        let Some(tab_entry) = model.tab_mut(tab) else {
            return;
        };
        tab_entry.visit_tracker.set_cause(Cause::Reload);
        if let Some(view) = tab_entry.page().view() {
            view.load(uri);
        }
    }

    pub(crate) fn open_outside(&mut self, tab: TabId, uri: String) {
        if let Err(error) = rikka_open::open(&uri) {
            tracing::warn!(%error, "failed to open address externally");
            return;
        }
        self.back_to_safety(tab);
    }

    pub(crate) fn reload_gone(&mut self, tab: TabId) {
        self.clear_failure(tab);
        let mut model = self.browser().edit();
        let Some(tab_entry) = model.tab_mut(tab) else {
            return;
        };
        tab_entry.visit_tracker.set_cause(Cause::Reload);
        match tab_entry.page().view().cloned() {
            Some(view) if view.is_open() => view.reload(),
            Some(view) => {
                let state = view.state();
                tab_entry.set_page(match state.is_empty() {
                    true => Page::Unloaded,
                    false => Page::Discarded(state.to_bytes().into()),
                });
                drop(model);
                self.changed();
            }
            None => {}
        }
    }

    pub(crate) fn continue_over_http(&mut self, tab: TabId, host: String, http: String) {
        self.clear_failure(tab);
        let Some(key) = self.model().tab(tab).map(|tab_entry| tab_entry.profile) else {
            return;
        };
        if let Err(error) = self.browser().allow_http(key, &host) {
            tracing::warn!(%error, "failed to load site over http");
            return;
        }
        let mut model = self.browser().edit();
        if let Some(tab_entry) = model.tab_mut(tab) {
            tab_entry.visit_tracker.set_cause(Cause::Link);
            if let Some(view) = tab_entry.page().view() {
                view.load(http);
            }
        }
    }

    pub(crate) fn back_to_safety(&mut self, tab: TabId) {
        self.clear_failure(tab);
        let mut model = self.browser().edit();
        let Some(tab_entry) = model.tab_mut(tab) else {
            return;
        };
        match tab_entry
            .page()
            .view()
            .filter(|view| view.page().can_go_back)
            .cloned()
        {
            Some(view) => {
                tab_entry.visit_tracker.set_cause(Cause::BackForward);
                view.go_back();
            }
            None => {
                tab_entry.meta.url.set(String::new());
                tab_entry.meta.title.set(String::new());
                tab_entry.set_page(Page::Unloaded);
            }
        }
        drop(model);
        self.changed();
    }

    pub(crate) fn wait_for_page(&mut self, tab: TabId) {
        let hang = self
            .browser()
            .page_services()
            .requests_mut()
            .edit_existing(tab)
            .and_then(|requests| requests.failure.take_hang());
        if let Some(hang) = hang {
            hang.wait();
        }
    }

    pub(crate) fn end_page(&mut self, tab: TabId) {
        let hang = self
            .browser()
            .page_services()
            .requests_mut()
            .edit_existing(tab)
            .and_then(|requests| requests.failure.take_hang());
        if let Some(hang) = hang {
            hang.stop_script();
        }
    }

    fn clear_failure(&mut self, tab: TabId) {
        if let Some(requests) = self
            .browser()
            .page_services()
            .requests_mut()
            .edit_existing(tab)
        {
            requests.failure.clear();
        }
    }

    pub(crate) fn listen(&mut self) {
        let released = {
            let model = self.model();
            self.browser()
                .page_services()
                .requests_mut()
                .release(|tab| model.tab(tab).is_none())
        };
        drop(released);
        let alive = self.browser().windows();
        let window = self.id();
        let page_services = self.browser().page_services();
        for (key, events) in page_services.subscribe_runtimes(window, &alive) {
            stream_keyed(
                (RUNTIME_FEED, key),
                events,
                move |shell: &mut Shell, event: RuntimeEvent| shell.runtime_event(key, event),
            );
        }
        if let Some(events) = page_services.subscribe_alerts(window, &alive) {
            stream_keyed(ALERT_FEED, events, |shell: &mut Shell, event| {
                shell.alert_event(event)
            });
        }
    }

    fn runtime_event(&mut self, key: ProfileKey, event: RuntimeEvent) {
        match event {
            RuntimeEvent::Download(download) => self.download_changed(download),
            RuntimeEvent::DownloadRemoved(id) => self.download_removed(&id),
            RuntimeEvent::Notification(notification) => self.notified(key, notification),
            RuntimeEvent::NotificationWithdrawn(name) => self.notification_closed(&name),
            RuntimeEvent::OpenWindow(request) => self.service_worker_opens(request),
            RuntimeEvent::Exited(exit) => {
                tracing::warn!(?exit, "web engine exited");
            }
            RuntimeEvent::Incompatible { protocol } => {
                tracing::error!(%protocol, "unsupported web engine protocol version");
            }
            other => self.extension_event(key, other),
        }
    }
}
