use std::sync::Arc;

use browser_pages::Cause;
use browser_preferences::ReaderStyle;
use browser_reader::{Article, EXTRACT_SCRIPT, parse, render};
use rikka_app::prelude::*;
use rikka_appearance::Appearance;
use rikka_web::{Event, Load, SessionSettings, World};
use serde_json::Value;

use crate::link::{self, Opening as LinkOpening};
use crate::omnibox::Opening;
use crate::reader::look::{labels, look};
use crate::reader::probe::WORLD;
use crate::reader::state::Reading;
use crate::shell::Shell;
use browser_model::TabId;

const EXTRACT: &str = "rikka-browser-reader-extract";
const EVENTS: &str = "rikka-browser-reader-events";
const LINKS: [&str; 3] = ["https://", "http://", "mailto:"];

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
            async move { view.evaluate(EXTRACT_SCRIPT, &World::isolated(WORLD)).await },
            move |shell: &mut Shell, answer: rikka_web::Result<Value>| {
                shell.extracted(tab, answer);
            },
        );
    }

    pub(crate) fn close_reader(&mut self) {
        if let Some(reading) = self.reader.close() {
            self.reader_closed(reading.tab);
        }
    }

    pub(crate) fn reader_closed(&mut self, tab: TabId) {
        cancel((EVENTS, tab));
        cancel((EXTRACT, tab));
    }

    pub(crate) fn restyle_reader(&mut self, style: ReaderStyle) {
        self.browser().preferences().set_reader_style(style);
        if let Some(reading) = self.reader.reading() {
            show(reading, style);
        }
    }

    fn extracted(&mut self, tab: TabId, answer: rikka_web::Result<Value>) {
        let article = match answer
            .map_err(|error| error.to_string())
            .and_then(|answer| parse(&answer).map_err(|error| error.to_string()))
        {
            Ok(article) => article,
            Err(error) => {
                tracing::debug!(%error, "failed to load reader view");
                self.reader.offer(tab, false);
                return;
            }
        };
        let active = self.active_tab();
        if active != Some(tab) {
            return;
        }
        let Some(key) = self.model().tab(tab).map(|tab_entry| tab_entry.profile) else {
            return;
        };
        let settings = SessionSettings::new()
            .javascript(false)
            .private(true)
            .handle_load_requests(true);
        let view = match self
            .browser()
            .engine(key)
            .and_then(|runtime| runtime.session(settings))
        {
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
                move |shell: &mut Shell, event: Event| {
                    shell.handle_reader_event(tab, event);
                },
            );
        }
        let dark = self.private() || Appearance::current().theme().is_dark();
        let reading = Reading {
            tab,
            view,
            article: Arc::new(article),
            dark,
        };
        show(&reading, self.browser().preferences().reader_style());
        self.reader.show(reading);
    }

    fn handle_reader_event(&mut self, tab: TabId, event: Event) {
        let Event::LoadRequest(request) = event else {
            return;
        };
        let uri = request.uri.clone();
        if !LINKS.iter().any(|scheme| uri.starts_with(scheme)) {
            request.allow();
            return;
        }
        let opening = link::opening(request.target);
        request.deny();
        match opening {
            LinkOpening::Here => {
                if self.reader.reads(tab) {
                    self.close_reader();
                }
                self.go(uri, Cause::Link);
            }
            LinkOpening::BackgroundTab => self.open_aside(uri),
            LinkOpening::ForegroundTab => self.open_address(uri, Opening::NewTab),
            LinkOpening::Window => self.open_address(uri, Opening::NewWindow),
        }
    }
}

fn show(reading: &Reading, style: ReaderStyle) {
    let article: &Article = &reading.article;
    let html = render(article, &look(style, reading.dark), &labels(article));
    reading.view.load(Load::html(html));
}
