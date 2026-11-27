use browser_store::Picture;
use browser_translators::MESSAGE;
use rikka_app::prelude::*;
use rikka_web::{CloseReason, Event, NewSessionRequest, Opening, PageState, SessionSettings};

use crate::little::{Little, PopupSize};
use crate::page::{Live, Opener, Page};
use crate::shell::{AdoptedSession, Shell, WindowContent, window};
use browser_model::{Favicon, Placement, TabId};

const FAVICON: &str = "rikka-browser-favicon";

impl Shell {
    pub(crate) fn handle_event(&mut self, tab: TabId, event: Event) {
        match event {
            Event::Changed => {
                if let Some(view) = self.view_of(tab) {
                    self.follow(tab, &view.page());
                }
            }
            Event::Started { .. } => self.started(tab),
            Event::Finished { .. } => self.probe_reader(tab),
            Event::Failed { url, error } => tracing::debug!(%url, %error, "page failed to load"),
            Event::NewSession(request) => self.new_session(tab, request),
            Event::Fullscreen(on) => self.page_fullscreen(tab, on),
            Event::ActivationRequested => self.select(tab),
            Event::ContextMenu(menu) => self.page_menu(tab, menu),
            Event::Message(message) if message.name == MESSAGE => {
                self.translation_message(tab, &message.body);
            }
            Event::Ask(ask) if ask.name == MESSAGE => self.translation_ask(tab, ask),
            Event::Closed(reason) => self.session_closed(tab, reason),
            _ => {}
        }
    }

    pub(crate) fn follow(&mut self, tab: TabId, page: &PageState) {
        let mut model = self.browser.edit();
        let Some(tab_entry) = model.tab_mut(tab) else {
            return;
        };
        let before = (
            (*tab_entry.meta.url).clone(),
            (*tab_entry.meta.title).clone(),
        );
        tab_entry.meta.follow(page);
        let moved = before.0 != *tab_entry.meta.url || before.1 != *tab_entry.meta.title;
        let fetch = match tab_entry.live_mut() {
            Some(live) if live.icon != page.favicon => {
                live.icon.clone_from(&page.favicon);
                match &page.favicon {
                    Some(_) => Some(live.view.clone()),
                    None => {
                        if !page.loading {
                            tab_entry.meta.favicon.set(None);
                        }
                        None
                    }
                }
            }
            _ => None,
        };
        model.remember_home(tab);
        drop(model);
        if let Some(view) = fetch {
            spawn_keyed(
                (FAVICON, tab),
                crate::favicons::fetched(view),
                move |shell: &mut Shell, fetched: Option<Favicon>| {
                    if let Some(favicon) = fetched {
                        shell.show_favicon(tab, favicon);
                    }
                },
            );
        }
        if moved {
            self.schedule_save();
        }
    }

    fn show_favicon(&mut self, tab: TabId, favicon: Favicon) {
        let mut model = self.browser.edit();
        let store = model.store().clone();
        let Some(tab_entry) = model.tab_mut(tab) else {
            return;
        };
        let page = (*tab_entry.meta.url).clone();
        if tab_entry.persistent_profile().is_some() && is_web(&page) {
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

    fn started(&mut self, tab: TabId) {
        self.reader_started(tab);
        self.translation_started(tab);
    }

    fn session_closed(&mut self, tab: TabId, reason: CloseReason) {
        self.page_left_fullscreen(tab);
        if self.peek.holds(tab) {
            self.close_peek();
            return;
        }
        let opener = {
            let model = self.model();
            let Some(live) = model.tab(tab).and_then(|tab_entry| tab_entry.page().live()) else {
                return;
            };
            if live.view.is_open() {
                return;
            }
            live.origin
        };
        match (reason, opener) {
            (CloseReason::Requested, _) => {}
            (CloseReason::Page, Opener::Page) => self.close(tab),
            (CloseReason::Page, _) => {
                if let Some(tab_entry) = self.browser.edit().tab_mut(tab) {
                    tab_entry.set_page(Page::Unloaded);
                }
                self.changed();
            }
            (CloseReason::EngineExited, _) => {
                if let Some(tab_entry) = self.browser.edit().tab_mut(tab) {
                    tab_entry.set_page(Page::Unloaded);
                }
            }
        }
    }

    pub(crate) fn new_session(&mut self, opener: TabId, request: NewSessionRequest) {
        let address = request.url.clone();
        let opening = request.opening;
        let Some(live) = adopt(request) else {
            return;
        };
        match opening {
            Opening::Popup { width, height } => {
                self.open_popup(opener, live, address, PopupSize { width, height })
            }
            Opening::Window if self.little() => {
                self.open_popup(opener, live, address, PopupSize::default())
            }
            Opening::Window => {
                let tab = self.browser.edit().adopt_tab(
                    self.id,
                    live,
                    address,
                    Placement::After(opener),
                    false,
                );
                if let Some(tab) = tab {
                    self.tear_off(tab);
                }
                self.changed();
            }
            Opening::ForegroundTab | Opening::BackgroundTab => {
                let focus = opening != Opening::BackgroundTab;
                if self.little() {
                    self.hand_to_browser(live, address, focus);
                    return;
                }
                if focus && self.peeks(opener, &address) {
                    self.peek_adopted(opener, live, address);
                    return;
                }
                live.view.set_visible(focus);
                self.browser.edit().adopt_tab(
                    self.id,
                    live,
                    address,
                    Placement::After(opener),
                    focus,
                );
                self.changed();
            }
        }
    }

    fn open_popup(&mut self, opener: TabId, live: Live, address: String, size: PopupSize) {
        let profile = self.model().tab(opener).map(|tab_entry| tab_entry.profile);
        let content = WindowContent::Little(
            profile,
            Little::Popup(AdoptedSession { live, address }, size),
        );
        open_window(window(self.browser(), self.private(), content));
    }

    pub(crate) fn adopt_opened(&mut self, live: Live, address: String, opening: Opening) {
        match opening {
            Opening::Popup { width, height } => {
                let profile = Some(self.model().profile_here(self.id));
                let content = WindowContent::Little(
                    profile,
                    Little::Popup(
                        AdoptedSession { live, address },
                        PopupSize { width, height },
                    ),
                );
                open_window(window(self.browser(), self.private(), content));
            }
            _ => {
                let focus = opening != Opening::BackgroundTab;
                live.view.set_visible(focus);
                self.browser
                    .edit()
                    .adopt_tab(self.id, live, address, Placement::Top, focus);
                self.changed();
            }
        }
    }
}

pub(crate) fn adopt(request: NewSessionRequest) -> Option<Live> {
    match request.open(SessionSettings::new()) {
        Ok(session) => Some(Live::adopt(session, Opener::Page)),
        Err(error) => {
            tracing::warn!(%error, "failed to open new page");
            None
        }
    }
}

fn is_web(address: &str) -> bool {
    address.starts_with("https://") || address.starts_with("http://")
}
