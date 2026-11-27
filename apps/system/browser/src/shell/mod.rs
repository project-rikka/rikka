mod autosave;
mod closing;
mod commands;
mod context;
mod editing;
pub(crate) mod events;
mod home;
mod navigation;
mod pages;
mod sync;
mod tabs;
mod view;

use std::cell::Ref;
use std::collections::HashMap;
use std::rc::Rc;

use rikka_app::prelude::*;
use rikka_web::Session;

use crate::boot::Boot;
use crate::browser::Browser;
use crate::geometry::WindowStyle;
use crate::internal::Pages;
use crate::internal::route::Presented;
use crate::menus::Menus;
use crate::model::Model;
use crate::page::Live;
use crate::sidebar::{Sidebar, titled};
use crate::topbar::Address;
use browser_model::{Placement, ProfileKey, Reopening, Row, TabId, Target, WindowId};
use browser_pages::Internal;

const NARROWEST: f32 = 640.0;
const SHORTEST: f32 = 420.0;

#[derive(State)]
pub(crate) struct Shell {
    #[untracked]
    browser: Rc<Browser>,
    #[untracked]
    id: WindowId,
    #[untracked]
    wired: HashMap<TabId, Session>,
    #[untracked]
    shown: Option<(TabId, Session)>,
    #[untracked]
    lit: Option<TabId>,
    #[untracked]
    page_shown: Option<Internal>,
    #[untracked]
    later: Vec<Reopening>,
    engine_error: Tracked<Option<String>>,
    #[state]
    sidebar: Sidebar,
    #[state]
    address: Address,
    #[state]
    menus: Menus,
    #[state]
    pages: Pages,
    #[state]
    pub(crate) command_bar: crate::command_bar::CommandBar,
    #[state]
    pub(crate) reader: crate::reader::Reader,
    #[state]
    pub(crate) sections: crate::sections::Sections,
    #[untracked]
    pub(crate) beside: Vec<(TabId, Session)>,
    #[state]
    pub(crate) gestures: crate::gestures::Gestures,
    #[state]
    pub(crate) peek: crate::peek::Peek,
    #[state]
    pub(crate) panes: crate::split::Panes,
    #[state]
    pub(crate) overlays: crate::overlays::Overlays,
    #[state]
    pub(crate) claim: crate::default_browser::Claim,
}

#[derive(Debug)]
pub(crate) struct AdoptedSession {
    pub(crate) live: Live,
    pub(crate) address: String,
}

#[derive(Debug)]
pub(crate) enum WindowContent {
    Blank,
    Addresses(Vec<String>),
    Tab(TabId),
    Adopted(AdoptedSession),
    Reopen(Reopening, Vec<String>),
    Start(Vec<String>),
    Little(Option<ProfileKey>, crate::little::Little),
}

pub(crate) fn window(
    browser: &Rc<Browser>,
    private: bool,
    content: WindowContent,
) -> Window<Shell> {
    let style = WindowStyle::for_content(&content);
    let browser = Rc::clone(browser);
    styled(
        Window::new(
            move |sender: Sender<Shell>| Shell::new(browser, sender, private, content),
            view::view,
        ),
        private,
        style,
    )
}

pub(crate) fn first_window(boot: Rc<Boot>, private: bool, content: WindowContent) -> Window<Shell> {
    let style = match &content {
        WindowContent::Start(_) if !private => WindowStyle::Main(boot.first_geometry()),
        content => WindowStyle::for_content(content),
    };
    styled(
        Window::new(
            move |sender: Sender<Shell>| Shell::new(boot.browser(), sender, private, content),
            view::view,
        ),
        private,
        style,
    )
}

fn styled(window: Window<Shell>, private: bool, style: WindowStyle) -> Window<Shell> {
    let window = crate::keys::bind(window)
        .title(tr!("browser.title"))
        .title_from(Shell::title)
        .min_size(NARROWEST, SHORTEST)
        .on_start(Shell::start)
        .on_focus(Shell::focused)
        .on_blur(Shell::blurred)
        .on_close_request(Shell::close_window)
        .on_command(rikka_ui::Command::Back, Shell::back)
        .on_command(rikka_ui::Command::Forward, Shell::forward);
    let window = style.apply(window);
    match private {
        true => window.theme(|_| Some(Theme::dark())),
        false => window,
    }
}

impl Shell {
    fn new(
        browser: Rc<Browser>,
        sender: Sender<Shell>,
        private: bool,
        content: WindowContent,
    ) -> Shell {
        let id = browser.edit().add_window(private);
        if let WindowContent::Little(..) = content {
            crate::little::mark(&mut browser.edit(), id);
        }
        browser.join(id, sender);
        let mut shell = Shell {
            browser,
            id,
            wired: HashMap::new(),
            shown: None,
            lit: None,
            page_shown: None,
            later: Vec::new(),
            engine_error: Tracked::new(None),
            sidebar: Sidebar::new(),
            address: Address::new(),
            menus: Menus::new(),
            pages: Pages::new(),
            command_bar: crate::command_bar::CommandBar::new(),
            reader: crate::reader::Reader::new(),
            sections: crate::sections::Sections::new(),
            beside: Vec::new(),
            gestures: crate::gestures::Gestures::new(),
            peek: crate::peek::Peek::new(),
            panes: crate::split::Panes::new(),
            overlays: crate::overlays::Overlays::new(),
            claim: crate::default_browser::Claim::new(),
        };
        match content {
            WindowContent::Blank => shell.open_blank(),
            WindowContent::Addresses(addresses) => shell.take_in(addresses),
            WindowContent::Tab(tab) => {
                shell
                    .browser
                    .edit()
                    .drop_row(id, Row::Tab(tab), Target::Today(0));
            }
            WindowContent::Adopted(AdoptedSession { live, address }) => {
                shell
                    .browser
                    .edit()
                    .adopt_tab(id, live, address, Placement::Top, true);
            }
            WindowContent::Reopen(reopening, addresses) => {
                shell.reopen_into(Some(reopening), addresses)
            }
            WindowContent::Start(addresses) => {
                let mut reopening = match private {
                    true => Vec::new(),
                    false => shell.browser.take_reopening().unwrap_or_default(),
                }
                .into_iter();
                let first = reopening.next();
                shell.later = reopening.collect();
                shell.reopen_into(first, addresses);
            }
            WindowContent::Little(profile, little) => shell.open_little(profile, little),
        }
        shell
    }

    fn reopen_into(&mut self, reopening: Option<Reopening>, addresses: Vec<String>) {
        if let Some(reopening) = reopening {
            self.browser.edit().reopen_window(self.id, &reopening);
        }
        if !addresses.is_empty() {
            self.take_in(addresses);
        } else if self.model().active(self.id).is_none() {
            self.open_blank();
        }
    }

    fn open_blank(&mut self) {
        self.browser
            .edit()
            .open_tab(self.id, None, Placement::Top, true);
        self.address.begin(String::new());
    }

    fn start(&mut self) {
        for reopening in std::mem::take(&mut self.later) {
            open_window(window(
                &self.browser,
                false,
                WindowContent::Reopen(reopening, Vec::new()),
            ));
        }
        self.changed();
        self.start_autosave();
        self.watch_memory();
        self.attend_default_browser();
    }

    fn focused(&mut self) {
        self.browser.focus(self.id, true);
        self.browser.edit().focused(self.id);
    }

    fn blurred(&mut self) {
        self.browser.focus(self.id, false);
        skip_view();
    }

    fn title(&self) -> String {
        let model = self.model();
        let page = match model.active(self.id) {
            Some(tab_entry) => match &*tab_entry.internal {
                Some(internal) => internal.title().to_string(),
                None if tab_entry.is_blank() => tr!("browser.tab.new").to_string(),
                None => titled(&tab_entry.meta.title, &tab_entry.meta.url),
            },
            None => tr!("browser.title").to_string(),
        };
        match self.private() {
            true => tr!("browser.private-title", title = &page).to_string(),
            false => page,
        }
    }

    pub(crate) fn browser(&self) -> &Rc<Browser> {
        &self.browser
    }

    pub(crate) fn id(&self) -> WindowId {
        self.id
    }

    pub(crate) fn active_tab(&self) -> Option<TabId> {
        self.model().window(self.id)?.active()
    }

    pub(crate) fn view_of(&self, tab: TabId) -> Option<Session> {
        self.model().tab(tab)?.page().view().cloned()
    }

    pub(crate) fn model(&self) -> Ref<'_, Model> {
        self.browser.model()
    }

    pub(crate) fn engine_error(&self) -> Option<&str> {
        self.engine_error.as_deref()
    }

    pub(crate) fn sidebar(&self) -> &Sidebar {
        &self.sidebar
    }

    pub(crate) fn address(&self) -> &Address {
        &self.address
    }

    pub(crate) fn address_mut(&mut self) -> &mut Address {
        &mut self.address
    }

    pub(crate) fn menus(&self) -> &Menus {
        &self.menus
    }

    pub(crate) fn pages(&self) -> &Pages {
        &self.pages
    }

    pub(crate) fn private(&self) -> bool {
        self.model()
            .window(self.id)
            .is_some_and(|layout| layout.private)
    }

    pub(crate) fn open_addresses(&mut self, addresses: Vec<String>, token: Option<String>) {
        self.take_in(addresses);
        self.changed();
        if let Some(token) = token {
            request_window(WindowRequest::Activate(token));
        }
    }

    pub(crate) fn raise(&self, target: WindowId, work: impl FnOnce(&mut Shell) + Send + 'static) {
        let Some(sender) = self.browser.sender(target) else {
            return;
        };
        request_window(WindowRequest::Raise(sender.window()));
        let _ = sender.send(work);
    }

    fn take_in(&mut self, addresses: Vec<String>) {
        let mut model = self.browser.edit();
        for address in addresses {
            model.open_tab(self.id, Some(address), Placement::Top, true);
        }
    }

    pub(crate) fn changed(&mut self) {
        self.browser.edit().tidy_splits();
        self.sync_tabs();
        self.browser.save_others(self.id);
        self.schedule_save();
        self.settle_bookmarks();
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        self.browser.leave(self.id);
    }
}

impl std::fmt::Debug for Shell {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Shell")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
