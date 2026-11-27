use std::sync::Arc;

use browser_preferences::ReaderStyle;
use browser_reader::{Article, render};
use rikka_app::prelude::*;
use rikka_appearance::Appearance;
use rikka_web::{Event, SessionSettings};

use crate::reader::look::{labels, look};
use crate::reader::state::Reading;
use crate::shell::Shell;
use browser_model::TabId;

const EXTRACT: &str = "rikka-browser-reader-extract";
const EVENTS: &str = "rikka-browser-reader-events";
const LINKS: [&str; 2] = ["https://", "http://"];

impl Shell {
    pub(crate) fn toggle_reader(&mut self) {
        let Some(tab) = self.active_tab() else {
            return;
        };
        if self.reader.reads(tab) {
            self.close_reader();
            return;
        }
        if !self.reader.offers(tab) {
            return;
        }
        let Some(view) = self.view_of(tab) else {
            return;
        };
        spawn_keyed(
            (EXTRACT, tab),
            browser_reader::extract(&view),
            move |shell: &mut Shell, article: browser_reader::Result<Article>| {
                shell.extracted(tab, article);
            },
        );
    }

    pub(crate) fn close_reader(&mut self) {
        if let Some(reading) = self.reader.close() {
            self.reader_closed(reading.tab);
            reading.view.close();
        }
    }

    pub(crate) fn reader_closed(&mut self, tab: TabId) {
        cancel((EVENTS, tab));
        cancel((EXTRACT, tab));
    }

    pub(crate) fn restyle_reader(&mut self, style: ReaderStyle) {
        self.browser().preferences().set_reader_style(style);
        let Some(reading) = self.reader.reading() else {
            return;
        };
        let document = document(&reading.article, style, reading.dark);
        let view = reading.view.clone();
        self.reader.restyle(document);
        view.reload();
    }

    fn extracted(&mut self, tab: TabId, article: browser_reader::Result<Article>) {
        let article = match article {
            Ok(article) => article,
            Err(error) => {
                tracing::debug!(%error, "failed to load reader view");
                self.reader.offer(tab, false);
                return;
            }
        };
        if self.active_tab() != Some(tab) {
            return;
        }
        let Some(key) = self.model().tab(tab).map(|tab_entry| tab_entry.profile) else {
            return;
        };
        let dark = self.private() || Appearance::current().theme().is_dark();
        let style = self.browser().preferences().reader_style();
        let document = document(&article, style, dark);
        let opened = self.browser().engine(key).and_then(|runtime| {
            runtime.session(SessionSettings::new().url(runtime.bundled_page(browser_reader::PAGE)))
        });
        let view = match opened {
            Ok(session) => session,
            Err(error) => {
                tracing::warn!(%error, "failed to open reader");
                return;
            }
        };
        if let Some(events) = view.events() {
            stream_keyed(
                (EVENTS, tab),
                events,
                move |shell: &mut Shell, event: Event| shell.handle_reader_event(tab, event),
            );
        }
        self.reader.show(Reading {
            tab,
            view,
            article: Arc::new(article),
            document,
            dark,
        });
    }

    fn handle_reader_event(&mut self, tab: TabId, event: Event) {
        match event {
            Event::Started { url } if LINKS.iter().any(|scheme| url.starts_with(scheme)) => {
                if self.reader.reads(tab) {
                    self.close_reader();
                }
                self.go_in(tab, url);
            }
            Event::Ask(ask) if ask.name == browser_reader::DOCUMENT => {
                if let Some(reading) = self.reader.reading().filter(|reading| reading.tab == tab) {
                    browser_reader::serve(ask, &reading.document);
                }
            }
            Event::NewSession(request) => self.new_session(tab, request),
            Event::Closed(_) if self.reader.reads(tab) => self.close_reader(),
            _ => {}
        }
    }
}

fn document(article: &Article, style: ReaderStyle, dark: bool) -> Arc<str> {
    Arc::from(render(article, &look(style, dark), &labels(article)))
}
