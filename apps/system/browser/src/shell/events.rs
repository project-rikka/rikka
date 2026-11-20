use browser_pages::{Arrival, Cause, IconCandidate, Visited, best_icon, is_recorded_scheme};
use browser_store::{Picture, TimeRange, Timestamp, Visit};
use rikka_app::prelude::*;
use rikka_web::{
    Event, HostVisitedQuery, Load, LoadRequest, NewSessionRequest, OpenLink, PageState, VisitFlags,
    VisitedQuery,
};

use crate::link::{self, Opening};
use crate::page::{Live, LiveIcon};
use crate::shell::sync::activate;
use crate::shell::{Shell, WindowContent, window};
use browser_model::Favicon;
use browser_model::{Placement, ProfileId, TabId};

const FAVICON: &str = "rikka-browser-favicon";
const KEPT_OPENINGS: usize = 8;

enum Referral {
    Opener,
    Load(Load),
}

enum FaviconUpdate {
    Fetch(String),
    Stored(Favicon),
    Same,
}

impl Shell {
    pub(crate) fn handle_event(&mut self, tab: TabId, event: Event) {
        match event {
            Event::Changed => {
                if let Some(page) = self.page_of(tab) {
                    self.follow(tab, &page);
                }
            }
            Event::PageStarted(uri) => self.started(tab, &uri),
            Event::PageStopped { .. } => self.stopped(tab),
            Event::LoadRequest(request) => self.decide(tab, request),
            Event::NewSession(request) => self.adopt_new_session(tab, request),
            Event::OpenLink(link) => self.follow_link(tab, link),
            Event::CloseRequested => self.close(tab),
            Event::Visit(visit) => {
                let visited = self.browser.edit().tab_mut(tab).and_then(|tab_entry| {
                    tab_entry
                        .visit_tracker
                        .visited(&visit.uri, arrival(visit.flags))
                });
                let profile = self
                    .model()
                    .tab(tab)
                    .and_then(|tab_entry| tab_entry.persistent_profile());
                let recorded = match (profile, visited) {
                    (Some(profile), Some(visited)) => {
                        self.record(profile, visited);
                        true
                    }
                    _ => false,
                };
                visit.answer(recorded);
            }
            Event::VisitedQuery(query) => self.visited_query(tab, query),
            Event::HostVisitedQuery(query) => self.host_visited_query(tab, query),
            Event::ScriptMessage(message)
                if crate::translate::is_translation_message(&message.name) =>
            {
                self.handle_translation_message(tab, message);
            }
            unanswered => self.handle_request(tab, unanswered),
        }
    }

    fn page_of(&self, tab: TabId) -> Option<PageState> {
        self.view_of(tab).map(|view| view.page())
    }

    pub(crate) fn follow(&mut self, tab: TabId, page: &PageState) {
        let translated = self.browser.translations().tabs().shows_translation(tab);
        let mut model = self.browser.edit();
        let store = model.store().clone();
        let Some(tab_entry) = model.tab_mut(tab) else {
            return;
        };
        let title = (*tab_entry.meta.title).clone();
        let url = (*tab_entry.meta.url).clone();
        let muted = tab_entry.page().live().is_some_and(|live| live.muted);
        tab_entry.meta.follow(page, muted);
        let moved = url != *tab_entry.meta.url || title != *tab_entry.meta.title;
        let rehosted =
            browser_pages::web_host(&url) != browser_pages::web_host(&tab_entry.meta.url);
        if let Some(profile) = tab_entry.persistent_profile()
            && is_recorded_scheme(&tab_entry.meta.url)
            && title != *tab_entry.meta.title
            && !tab_entry.meta.title.is_empty()
            && !translated
        {
            store.set_title(
                profile,
                (*tab_entry.meta.url).clone(),
                (*tab_entry.meta.title).clone(),
            );
        }
        let navigated = url != *tab_entry.meta.url;
        let shown = match (
            tab_entry.live_mut(),
            best_icon(
                &page.uri,
                page.icons.iter().map(|icon| IconCandidate {
                    url: &icon.url,
                    rel: &icon.rel,
                    sizes: &icon.sizes,
                    mime_type: icon.mime_type.as_deref(),
                }),
                !page.loading,
            ),
        ) {
            (Some(live), Some(icon)) => match &live.icon {
                Some(live_icon) if live_icon.url == icon => match (&live_icon.favicon, navigated) {
                    (Some(favicon), true) => FaviconUpdate::Stored(favicon.clone()),
                    _ => FaviconUpdate::Same,
                },
                _ => {
                    live.icon = Some(LiveIcon {
                        url: icon.clone(),
                        favicon: None,
                    });
                    FaviconUpdate::Fetch(icon)
                }
            },
            _ => FaviconUpdate::Same,
        };
        model.remember_home(tab);
        drop(model);
        match shown {
            FaviconUpdate::Fetch(icon) => self.fetch_favicon(tab, icon),
            FaviconUpdate::Stored(favicon) => self.show_favicon(tab, favicon),
            FaviconUpdate::Same => {}
        }
        if rehosted {
            self.fit_zoom(tab);
        }
        if moved {
            self.schedule_save();
        }
    }

    fn fetch_favicon(&mut self, tab: TabId, icon: String) {
        let fetched = icon.clone();
        spawn_keyed(
            (FAVICON, tab),
            crate::favicons::fetched(fetched, self.color_scheme()),
            move |shell: &mut Shell, fetched: Option<Favicon>| {
                if let Some(favicon) = fetched {
                    shell.favicon_fetched(tab, &icon, favicon);
                }
            },
        );
    }

    fn favicon_fetched(&mut self, tab: TabId, icon: &str, favicon: Favicon) {
        {
            let mut model = self.browser.edit();
            let Some(live_icon) = model
                .tab_mut(tab)
                .and_then(|tab_entry| tab_entry.live_mut())
                .and_then(|live| live.icon.as_mut())
                .filter(|live_icon| live_icon.url == icon)
            else {
                return;
            };
            live_icon.favicon = Some(favicon.clone());
        }
        self.show_favicon(tab, favicon);
    }

    fn show_favicon(&mut self, tab: TabId, favicon: Favicon) {
        let mut model = self.browser.edit();
        let store = model.store().clone();
        let Some(tab_entry) = model.tab_mut(tab) else {
            return;
        };
        let page = (*tab_entry.meta.url).clone();
        if tab_entry.persistent_profile().is_some() && is_recorded_scheme(&page) {
            match Picture::new(favicon.width, favicon.height, favicon.rgba.to_vec()) {
                Ok(picture) => store.set_favicon(page, picture),
                Err(error) => tracing::debug!(%error, "failed to store favicon"),
            }
        }
        if tab_entry.meta.favicon.as_ref() != Some(&favicon) {
            tab_entry.meta.favicon.set(Some(favicon));
        }
        model.remember_home(tab);
    }

    fn started(&mut self, tab: TabId, uri: &str) {
        if let Some(tab_entry) = self.browser.edit().tab_mut(tab) {
            tab_entry.visit_tracker.navigation_started();
            tab_entry.meta.tooltip.set(None);
        }
        self.page_started(tab, uri);
        self.reader_started(tab);
        self.translation_started(tab);
    }

    fn stopped(&mut self, tab: TabId) {
        self.probe_reader(tab);
        self.capture_state(tab);
    }

    fn record(&self, profile: ProfileId, visited: Visited) {
        let Visited { url, transition } = visited;
        self.model()
            .store()
            .record_visit(profile, Visit::new(url, transition));
    }

    fn visited_query(&mut self, tab: TabId, query: VisitedQuery) {
        let Some(profile) = self
            .model()
            .tab(tab)
            .and_then(|tab_entry| tab_entry.persistent_profile())
        else {
            let unvisited = vec![false; query.uris.len()];
            query.answer(unvisited);
            return;
        };
        let store = self.browser.store();
        spawn_blocking(
            move || match store.visited(profile, &query.uris) {
                Ok(visited) => query.answer(visited),
                Err(error) => tracing::debug!(%error, "failed to read visited links"),
            },
            |_: &mut Shell, (): ()| {},
        );
    }

    fn host_visited_query(&mut self, tab: TabId, query: HostVisitedQuery) {
        let Some(profile) = self
            .model()
            .tab(tab)
            .and_then(|tab_entry| tab_entry.persistent_profile())
        else {
            query.answer(false);
            return;
        };
        let store = self.browser.store();
        spawn_blocking(
            move || {
                let range = TimeRange::new(
                    Timestamp::from_unix_millis(query.after),
                    Timestamp::from_unix_millis(query.before),
                );
                let visited = store.host_visited(profile, &query.host, range);
                query.answer(visited.unwrap_or(false));
            },
            |_: &mut Shell, (): ()| {},
        );
    }

    fn decide(&mut self, tab: TabId, request: LoadRequest) {
        let opening = link::opening(request.target);
        let elsewhere = request.target.opens_new_session();
        match opening {
            Opening::BackgroundTab | Opening::ForegroundTab => {
                let uri = request.uri.clone();
                request.deny();
                self.open_link(tab, uri, opening, Referral::Opener);
            }
            Opening::Here if !elsewhere && self.peeks(tab, &request) => {
                let uri = request.uri.clone();
                request.deny();
                self.peek_from(tab, uri);
            }
            Opening::Here if !elsewhere => {
                if let Some(tab_entry) = self.browser.edit().tab_mut(tab) {
                    tab_entry
                        .visit_tracker
                        .policy_decided(&request.uri, request.user_gesture);
                }
                request.allow();
            }
            Opening::Here | Opening::Window => {
                if let Some(live) = self
                    .browser
                    .edit()
                    .tab_mut(tab)
                    .and_then(|tab_entry| tab_entry.live_mut())
                {
                    if live.openings.len() == KEPT_OPENINGS {
                        live.openings.pop_front();
                    }
                    live.openings.push_back((request.uri.clone(), opening));
                }
                request.allow();
            }
        }
    }

    fn follow_link(&mut self, opener: TabId, link: OpenLink) {
        match link::opening(link.target) {
            Opening::Here => {
                if let Some(view) = self.view_of(opener) {
                    view.load(link.load());
                }
            }
            Opening::Window => open_window(window(
                self.browser(),
                self.private(),
                WindowContent::Addresses(vec![link.uri]),
            )),
            opening => {
                let load = link.load();
                self.open_link(opener, link.uri, opening, Referral::Load(load));
            }
        }
    }

    fn open_link(&mut self, opener: TabId, uri: String, opening: Opening, referral: Referral) {
        if self.little_link(&uri, opening) {
            return;
        }
        let focus = opening != Opening::BackgroundTab;
        let opened = {
            let mut model = self.browser.edit();
            let opened = model.open_tab(self.id, Some(uri), Placement::After(opener), focus);
            if let Some(tab_entry) = opened.and_then(|tab| model.tab_mut(tab)) {
                tab_entry.visit_tracker.set_cause(Cause::Link);
            }
            opened
        };
        if let Some(opened) = opened {
            match referral {
                Referral::Opener => self.refer(opened, opener),
                Referral::Load(load) => self.refer_with(opened, load),
            }
            if !focus {
                self.activate_in_background(opened);
            }
        }
        self.changed();
    }

    fn activate_in_background(&mut self, tab: TabId) {
        if let Ok(Some((view, activation))) = self.browser.activate(tab) {
            view.set_visible(false);
            activate(&view, activation, self.referred_load(tab));
        }
    }

    fn take_opening(&mut self, opener: TabId, uri: &str) -> Opening {
        let mut model = self.browser.edit();
        let Some(live) = model
            .tab_mut(opener)
            .and_then(|tab_entry| tab_entry.live_mut())
        else {
            return Opening::Here;
        };
        match live
            .openings
            .iter()
            .rposition(|(candidate, _)| candidate == uri)
        {
            Some(at) => live
                .openings
                .remove(at)
                .map_or(Opening::Here, |(_, opening)| opening),
            None => Opening::Here,
        }
    }

    fn adopt_new_session(&mut self, opener: TabId, request: NewSessionRequest) {
        let uri = request.uri.clone();
        let popup = request.popup;
        let opening = self.take_opening(opener, &uri);
        let session = match request.open() {
            Ok(session) => session,
            Err(error) => {
                tracing::warn!(%error, "failed to open new window");
                return;
            }
        };
        let live = Live::adopt(session);
        if let Some(key) = self.model().tab(opener).map(|tab_entry| tab_entry.profile) {
            self.browser.dress(key, &live.view);
        }
        let Some(live) = self.route_new_window(opener, live, &uri, opening, popup) else {
            return;
        };
        let tab = {
            let mut model = self.browser.edit();
            let tab = model.adopt_tab(self.id, live, uri, Placement::After(opener), true);
            if let Some(tab_entry) = tab.and_then(|tab| model.tab_mut(tab)) {
                tab_entry.visit_tracker.set_cause(Cause::Link);
            }
            tab
        };
        if tab.is_some() {
            self.changed();
        }
    }
}

fn arrival(flags: VisitFlags) -> Arrival {
    Arrival {
        top_level: flags.contains(VisitFlags::TOP_LEVEL),
        failed: flags.contains(VisitFlags::UNRECOVERABLE_ERROR),
        redirect_source: flags.contains(VisitFlags::REDIRECT_SOURCE)
            || flags.contains(VisitFlags::REDIRECT_SOURCE_PERMANENT),
        redirect_target: flags.contains(VisitFlags::REDIRECT_TEMPORARY)
            || flags.contains(VisitFlags::REDIRECT_PERMANENT),
    }
}
