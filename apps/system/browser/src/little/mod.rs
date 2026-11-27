mod route;
mod view;

pub(crate) use route::{Routing, routing, wants_little};
pub(crate) use view::{caption_bar, view};

use rikka_app::prelude::*;

use crate::command::Command;
use crate::model::Model;
use crate::page::{Live, Page};
use crate::shell::{AdoptedSession, Shell, WindowContent, window};
use browser_model::{Home, Placement, ProfileKey, Row, Saved, Target, WindowId};
use browser_pages::Internal;

const WIDTH: f32 = 900.0;
const HEIGHT: f32 = 640.0;
const SMALLEST: f32 = 100.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PopupSize {
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
}

#[derive(Debug)]
pub(crate) enum Little {
    Handed(String),
    Popup(AdoptedSession, PopupSize),
}

impl Little {
    pub(crate) fn size(&self) -> (f32, f32) {
        let requested = match self {
            Little::Popup(_, size) => *size,
            Little::Handed(_) => PopupSize::default(),
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
        let id = model.insert_tab(profile, Home::Today(None), Saved::new(address, ""));
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

    pub(crate) fn hand_to_browser(&mut self, live: Live, address: String, focus: bool) {
        let private = self.private();
        let Some(target) = self.browser_window(private) else {
            let content = WindowContent::Adopted(AdoptedSession { live, address });
            open_window(window(self.browser(), private, content));
            return;
        };
        live.view.set_visible(false);
        self.browser()
            .edit()
            .adopt_tab(target, live, address, Placement::Top, focus);
        match focus {
            true => self.raise(target, Shell::changed),
            false => {
                if let Some(sender) = self.browser().sender(target) {
                    let _ = sender.send(Shell::changed);
                }
            }
        }
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

    pub(crate) fn open_in_browser_window(&mut self, url: String) {
        let private = self.private();
        match self.browser_window(private) {
            Some(target) => self.raise(target, move |shell: &mut Shell| shell.open_page(url)),
            None => {
                let content = WindowContent::Addresses(vec![url]);
                open_window(window(self.browser(), private, content));
            }
        }
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
