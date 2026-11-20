mod route;
mod view;

pub(crate) use route::{Routing, opens_popup, routing, wants_little};
pub(crate) use view::{caption_bar, view};

use rikka_app::prelude::*;
use rikka_web::Popup;

use crate::command::Command;
use crate::link::Opening;
use crate::model::Model;
use crate::page::{AdoptedSession, Live, Page};
use crate::shell::{Shell, WindowContent, window};
use browser_model::{Home, ProfileKey, Row, TabId, Target, WindowId};
use browser_pages::Internal;

const WIDTH: f32 = 900.0;
const HEIGHT: f32 = 640.0;
const SMALLEST: f32 = 100.0;

#[derive(Debug)]
pub(crate) enum Little {
    Handed(String),
    Popup(AdoptedSession, Option<Popup>),
}

impl Little {
    pub(crate) fn size(&self) -> (f32, f32) {
        let requested = match self {
            Little::Popup(_, Some(popup)) => *popup,
            _ => Popup::default(),
        };
        let width = requested.width.map_or(WIDTH, |width| width as f32);
        let height = requested
            .height
            .map_or(HEIGHT, |height| height as f32 + CaptionBar::HEIGHT);
        (width.max(SMALLEST), height.max(SMALLEST))
    }
}

pub(crate) fn mark(model: &mut Model, window: WindowId) {
    if let Some(layout) = model.windows.get_mut(window) {
        layout.little = true;
        layout.set_space(None);
    }
}

impl Shell {
    pub(crate) fn little(&self) -> bool {
        self.model()
            .window(self.id())
            .is_some_and(|layout| layout.little)
    }

    pub(crate) fn open_little(&mut self, opener: Option<ProfileKey>, little: Little) {
        let window = self.id();
        let browser = std::rc::Rc::clone(self.browser());
        let mut model = browser.edit();
        let profile = match (
            opener,
            model.window(window).is_some_and(|layout| layout.private),
        ) {
            (Some(profile), _) => profile,
            (None, true) => ProfileKey::Private,
            (None, false) => ProfileKey::Persistent(model.profiles.list()[0].id),
        };
        let (address, page) = match little {
            Little::Handed(address) => (address, Page::Unloaded),
            Little::Popup(AdoptedSession { live, address }, _) => (address, Page::Live(live)),
        };
        let id = model.insert_linked_tab(profile, Home::Today(None), address);
        if let Some(tab_entry) = model.tab_mut(id) {
            tab_entry.set_page(page);
        }
        if let Some(layout) = model.windows.get_mut(window) {
            layout.insert_today(None, 0, id);
            layout.set_active(Some(id));
        }
    }

    pub(crate) fn little_command(&mut self, command: Command) -> bool {
        if !self.little() {
            return false;
        }
        match routing(command) {
            Routing::Here => return false,
            Routing::Close => request_window(WindowRequest::Close),
            Routing::Browser(internal) => self.open_in_browser(internal),
            Routing::Ignore => skip_view(),
        }
        true
    }

    fn open_in_browser(&mut self, internal: Internal) {
        let private = self.private();
        match self.browser_window(private) {
            Some(target) => {
                self.raise(target, move |shell: &mut Shell| {
                    shell.open_internal(internal)
                });
            }
            None => {
                let content = WindowContent::Addresses(vec![internal.address()]);
                open_window(window(self.browser(), private, content));
            }
        }
    }

    pub(crate) fn little_link(&mut self, uri: &str, opening: Opening) -> bool {
        if !self.little() || opening == Opening::Window {
            return false;
        }
        let private = self.private();
        let uri = uri.to_owned();
        match self.browser_window(private) {
            Some(target) => self.raise(target, move |shell: &mut Shell| shell.open_aside(uri)),
            None => open_window(window(
                self.browser(),
                private,
                WindowContent::Addresses(vec![uri]),
            )),
        }
        true
    }

    pub(crate) fn route_new_window(
        &mut self,
        opener: TabId,
        live: Live,
        address: &str,
        opening: Opening,
        popup: Option<Popup>,
    ) -> Option<Live> {
        if popup.is_some() || opens_popup(opening) || self.little() {
            let profile = self.model().tab(opener).map(|tab_entry| tab_entry.profile);
            let private = self.private();
            let content = WindowContent::Little(
                profile,
                Little::Popup(
                    AdoptedSession {
                        live,
                        address: address.to_owned(),
                    },
                    popup,
                ),
            );
            open_window(window(self.browser(), private, content));
            return None;
        }
        let peeking = {
            let model = self.model();
            model.tab(opener).is_some_and(|tab_entry| {
                crate::peek::peeks_from(tab_entry.home)
                    && opening == Opening::Here
                    && crate::peek::crosses(&tab_entry.meta.url, address)
            })
        };
        if peeking && !self.peek.holds(opener) {
            self.peek_adopted(opener, live, address.to_owned());
            return None;
        }
        Some(live)
    }

    pub(crate) fn move_into_browser(&mut self) {
        let Some(tab) = self.active_tab() else {
            return;
        };
        let private = self.private();
        let target = self.browser_window(private);
        match target {
            Some(target) => {
                let moved = self
                    .browser()
                    .edit()
                    .drop_row(target, Row::Tab(tab), Target::Today(0));
                if moved {
                    self.raise(target, Shell::changed);
                }
                self.changed();
            }
            None => {
                let browser = std::rc::Rc::clone(self.browser());
                if !private {
                    for reopening in browser.take_reopening().unwrap_or_default() {
                        open_window(window(
                            &browser,
                            false,
                            WindowContent::Reopen(reopening, Vec::new()),
                        ));
                    }
                }
                open_window(window(&browser, private, WindowContent::Tab(tab)));
            }
        }
    }

    pub(crate) fn close_if_emptied(&mut self) -> bool {
        let emptied = self
            .model()
            .window(self.id())
            .is_some_and(|layout| layout.little && layout.tabs().is_empty());
        if emptied {
            request_window(WindowRequest::Close);
        }
        emptied
    }

    fn browser_window(&self, private: bool) -> Option<WindowId> {
        let model = self.model();
        match private {
            false => model.windows.main(),
            true => model
                .windows
                .iter()
                .filter(|layout| layout.private && !layout.little)
                .last()
                .map(|layout| layout.id),
        }
    }
}
