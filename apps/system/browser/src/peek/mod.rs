mod site;

pub(crate) use site::{crosses, peeks_from, wants_peek};

use std::rc::Rc;

use rikka_app::prelude::*;
use rikka_input::Chord;
use rikka_web::{Event, Load, LoadRequest};
use rikka_widgets::row;

use crate::browser::Browser;
use crate::content::tab_page;
use crate::model::Tab;
use crate::page::{Live, Page};
use crate::shell::Shell;
use crate::widgets::{favicon, glyph_button};
use browser_model::Arrangement;
use browser_model::{Home, TabId};

const WIRE: &str = "rikka-browser-peek";
const SHARE: f32 = 0.8;
const HEADER: f32 = 40.0;
const GLYPH: f32 = 16.0;

struct Floating {
    browser: Rc<Browser>,
    tab: TabId,
}

impl Drop for Floating {
    fn drop(&mut self) {
        let mut model = self.browser.edit();
        if model.windows.window_of(self.tab).is_none() {
            model.tabs.remove(self.tab);
        }
    }
}

#[derive(State)]
pub(crate) struct Peek {
    #[untracked]
    floating: Option<Floating>,
    shown: Tracked<Option<TabId>>,
}

impl Peek {
    pub(crate) fn new() -> Peek {
        Peek {
            floating: None,
            shown: Tracked::new(None),
        }
    }

    pub(crate) fn shown(&self) -> Option<TabId> {
        *self.shown
    }

    pub(crate) fn holds(&self, tab: TabId) -> bool {
        self.floating
            .as_ref()
            .is_some_and(|floating| floating.tab == tab)
    }
}

impl std::fmt::Debug for Peek {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Peek")
            .field("shown", &*self.shown)
            .finish_non_exhaustive()
    }
}

impl Shell {
    pub(crate) fn peek(&mut self, url: String) {
        let opener = self.active_tab();
        if let Some(opener) = opener {
            self.peek_from(opener, url);
        }
    }

    pub(crate) fn peeks(&self, tab: TabId, request: &LoadRequest) -> bool {
        if self.peek.holds(tab) {
            return false;
        }
        let model = self.model();
        model
            .tab(tab)
            .is_some_and(|tab_entry| wants_peek(tab_entry.home, &tab_entry.meta.url, request))
    }

    pub(crate) fn peek_from(&mut self, opener: TabId, url: String) {
        let Some((key, home)) = self.model().tab(opener).map(|tab_entry| {
            let space = self
                .model()
                .window(self.id())
                .and_then(|layout| layout.space());
            (tab_entry.profile, Home::Today(space))
        }) else {
            return;
        };
        self.close_peek();
        let browser = Rc::clone(self.browser());
        let tab = browser.edit().insert_linked_tab(key, home, url.clone());
        let floating = Floating {
            browser: Rc::clone(&browser),
            tab,
        };
        let opened = browser.engine(key).and_then(|runtime| Live::open(&runtime));
        let live = match opened {
            Ok(live) => live,
            Err(error) => {
                tracing::warn!(%error, "failed to open peek");
                return;
            }
        };
        browser.dress(key, &live.view);
        let load = match self.view_of(opener) {
            Some(source) => Load::uri(url).referrer_session(&source),
            None => Load::uri(url),
        };
        live.view.load(load);
        self.show_peek(floating, live);
    }

    pub(crate) fn peek_adopted(&mut self, opener: TabId, live: Live, url: String) {
        let Some((key, space)) = self.model().tab(opener).map(|tab_entry| {
            let space = self
                .model()
                .window(self.id())
                .and_then(|layout| layout.space());
            (tab_entry.profile, space)
        }) else {
            return;
        };
        self.close_peek();
        let browser = Rc::clone(self.browser());
        let tab = browser
            .edit()
            .insert_linked_tab(key, Home::Today(space), url);
        let floating = Floating {
            browser: Rc::clone(&browser),
            tab,
        };
        self.show_peek(floating, live);
    }

    fn show_peek(&mut self, floating: Floating, live: Live) {
        let tab = floating.tab;
        let events = live.events().stream();
        if let Some(tab_entry) = self.browser().edit().tab_mut(tab) {
            tab_entry.set_page(Page::Live(live));
        }
        self.peek.floating = Some(floating);
        self.peek.shown.set(Some(tab));
        stream_keyed(
            (WIRE, tab),
            events,
            move |shell: &mut Shell, event: Event| match event {
                Event::CloseRequested => shell.close_peek(),
                event => shell.handle_event(tab, event),
            },
        );
    }

    pub(crate) fn peek_key(&mut self, key: Key, modifiers: Modifiers) -> bool {
        if key != Key::Escape || modifiers != Modifiers::NONE || self.peek.shown().is_none() {
            return false;
        }
        self.close_peek();
        true
    }

    pub(crate) fn close_peek(&mut self) {
        if let Some(tab) = self.peek.shown() {
            cancel((WIRE, tab));
        }
        self.peek.floating = None;
        if self.peek.shown().is_some() {
            self.peek.shown.set(None);
        }
    }

    pub(crate) fn open_peek_as_tab(&mut self) {
        if let Some(tab) = self.promote_peek() {
            self.select(tab);
        }
    }

    pub(crate) fn open_peek_in_split(&mut self) {
        let anchor = self.active_tab();
        let Some(tab) = self.promote_peek() else {
            return;
        };
        match anchor {
            Some(anchor) => {
                self.browser().edit().split_beside(
                    self.id(),
                    anchor,
                    tab,
                    Arrangement::SideBySide,
                    true,
                );
                self.select(tab);
            }
            None => self.select(tab),
        }
    }

    fn promote_peek(&mut self) -> Option<TabId> {
        let tab = self.peek.shown()?;
        let window = self.id();
        {
            let mut model = self.browser().edit();
            let layout = model.window(window)?;
            let space = layout.space();
            let at = layout
                .active()
                .and_then(|active| layout.place_in_today(active))
                .filter(|(candidate, _)| *candidate == space)
                .map_or(0, |(_, at)| at + 1);
            let profile = model.profile_here(window);
            model.move_tab_home(tab, Home::Today(space), profile);
            model.windows.get_mut(window)?.insert_today(space, at, tab);
        }
        cancel((WIRE, tab));
        self.peek.floating = None;
        self.peek.shown.set(None);
        Some(tab)
    }
}

pub(crate) fn reserved(shell: &Shell, tab: TabId) -> Option<Chord> {
    shell
        .peek
        .holds(tab)
        .then(|| Chord::new(Modifiers::NONE, Key::Escape))
}

pub(crate) fn view(shell: &Shell) -> Option<Element> {
    let tab = shell.peek.shown()?;
    let model = shell.model();
    let tab_entry = model.tab(tab)?;
    let card = container(
        column_of([
            header(tab_entry),
            tab_page(shell, tab_entry).grow(1.0).fill_width(),
        ])
        .fill(),
    )
    .background(palette().background)
    .border(1.0, palette().outline.with_alpha(0.5))
    .radius(Corner::Panel)
    .clipped()
    .width(Length::portion(SHARE))
    .height(Length::portion(SHARE))
    .build()
    .transition(fade_scale());
    Some(
        column_of([
            scrim()
                .on_dismiss(|shell: &mut Shell| shell.close_peek())
                .build(),
            card,
        ])
        .align_items(Align::Center)
        .justify(Justify::Center)
        .fill()
        .transition(fade())
        .keyed(("peek", tab)),
    )
}

fn header(tab_entry: &Tab) -> Element {
    let shown = crate::address::display_address(&tab_entry.meta.url);
    let title = match tab_entry.meta.title.trim() {
        "" => shown.host.clone(),
        title => title.to_owned(),
    };
    row! {
        favicon(tab_entry.meta.favicon.as_ref(), MaterialSymbol::PUBLIC, GLYPH);
        column_of([
            text(title).label().no_wrap().any(),
            row! {
                crate::site_info::peek_badge(tab_entry.id, &tab_entry.meta.security, 12.0);
                text(shown.host).caption().muted().no_wrap().shrink(1.0).min_width(0.0);
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .any(),
        ])
        .grow(1.0)
        .min_width(0.0);
        button(text(tr!("browser.peek.open-tab")))
            .ghost()
            .small()
            .focusless()
            .on_press(|shell: &mut Shell| shell.open_peek_as_tab());
        glyph_button(MaterialSymbol::SPLITSCREEN_RIGHT, GLYPH)
            .on_press(|shell: &mut Shell| shell.open_peek_in_split());
        glyph_button(MaterialSymbol::CLOSE, GLYPH)
            .on_press(|shell: &mut Shell| shell.close_peek());
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::NORMAL, Space::NONE))
    .height(HEADER)
    .shrink(0.0)
    .fill_width()
    .any()
}
