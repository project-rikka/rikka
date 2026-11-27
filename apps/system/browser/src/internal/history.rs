use std::collections::{BTreeMap, HashSet};
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use browser_store::Timestamp;
use futures_util::future::Either;
use jiff::civil::Date;
use rikka_app::prelude::*;
use rikka_web::{History, HistoryItem, HistoryQuery};
use rikka_widgets::{column, row};

use crate::internal::Span;
use crate::internal::days;
use crate::internal::status::Status;
use crate::menus::{Menus, Subject};
use crate::shell::Shell;
use crate::widgets::glyph_button;

const PAGE: u32 = 100;
const AHEAD: usize = 30;
const ROW: f32 = 44.0;
const HEADER: f32 = 36.0;
const WIDEST: f32 = 860.0;
const FETCH: &str = "rikka-browser-history";
const FORGET: &str = "rikka-browser-history-forget";
const SEARCH: &str = "rikka-browser-history-search";
const SEARCH_PAUSE: Duration = Duration::from_millis(150);
const REFRESH: &str = "rikka-browser-history-refresh";
const REFRESH_PAUSE: Duration = Duration::from_secs(2);
pub(crate) const CLEAR_BUTTON: &str = "rikka-browser-history-clear";

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Item {
    Day(Arc<str>),
    Visit(Arc<HistoryItem>),
}

#[derive(State)]
pub(crate) struct HistoryPage {
    #[untracked]
    history: Option<History>,
    #[untracked]
    search: TextDocument,
    #[untracked]
    until: Option<SystemTime>,
    #[untracked]
    seen: HashSet<String>,
    #[untracked]
    last_day: Option<Date>,
    #[untracked]
    waiting: bool,
    #[untracked]
    exhausted: bool,
    items: Tracked<Arc<Vec<Item>>>,
    chosen: Tracked<BTreeMap<String, String>>,
    status: Tracked<Status>,
}

impl HistoryPage {
    pub(crate) fn new() -> HistoryPage {
        HistoryPage {
            history: None,
            search: TextDocument::new(""),
            until: None,
            seen: HashSet::new(),
            last_day: None,
            waiting: false,
            exhausted: false,
            items: Tracked::new(Arc::new(Vec::new())),
            chosen: Tracked::new(BTreeMap::new()),
            status: Tracked::new(Status::Waiting),
        }
    }

    pub(crate) fn open(&mut self, history: rikka_web::Result<History>) {
        self.history = match history {
            Ok(history) => Some(history),
            Err(error) => {
                tracing::warn!(%error, "failed to start Chrome for history");
                None
            }
        };
        self.refresh();
    }

    pub(crate) fn changed(&mut self) {
        after_keyed(REFRESH, REFRESH_PAUSE, |page: &mut HistoryPage| {
            page.refresh()
        });
    }

    fn refresh(&mut self) {
        self.until = None;
        self.seen.clear();
        self.last_day = None;
        self.waiting = false;
        self.exhausted = false;
        self.items.set(Arc::new(Vec::new()));
        if !self.chosen.is_empty() {
            self.chosen.set(BTreeMap::new());
        }
        self.status.set(Status::Waiting);
        self.fetch();
    }

    fn fetch(&mut self) {
        let Some(history) = self.history.clone() else {
            cancel(FETCH);
            self.status.set(Status::Failed);
            return;
        };
        if self.waiting || self.exhausted {
            return;
        }
        self.waiting = true;
        let mut query = HistoryQuery::new(self.search.text()).limit(PAGE);
        if let Some(until) = self.until {
            query = query.until(until);
        }
        spawn_keyed(
            FETCH,
            history.search(&query),
            move |page: &mut HistoryPage, found: rikka_web::Result<Vec<HistoryItem>>| {
                page.loaded(found);
            },
        );
    }

    fn loaded(&mut self, found: rikka_web::Result<Vec<HistoryItem>>) {
        self.waiting = false;
        let entries = match found {
            Ok(entries) => entries,
            Err(error) => {
                tracing::warn!(%error, "failed to read history");
                self.status.set(Status::Failed);
                return;
            }
        };
        self.exhausted = entries.len() < PAGE as usize;
        self.until = entries
            .iter()
            .filter_map(|entry| entry.last_visit_time)
            .min()
            .or(self.until);
        let today = days::today();
        let mut items = Vec::clone(&self.items);
        let before = items.len();
        for entry in entries {
            if !self.seen.insert(entry.id.clone()) {
                continue;
            }
            let date = entry
                .last_visit_time
                .and_then(|at| days::local_time(Timestamp::from(at)))
                .map(|local_time| local_time.date);
            if date.is_some() && date != self.last_day {
                self.last_day = date;
                if let Some(date) = date {
                    items.push(Item::Day(days::day(date, today).into()));
                }
            }
            items.push(Item::Visit(Arc::new(entry)));
        }
        if items.len() == before {
            self.exhausted = true;
        }
        self.items.set(Arc::new(items));
        self.status.set(Status::Ready);
    }

    fn showing(&mut self, shown: Range<usize>) {
        if shown.end + AHEAD >= self.items.len() && !self.exhausted && !self.waiting {
            self.fetch();
        } else {
            skip_view();
        }
    }

    fn searched(&mut self) {
        after_keyed(SEARCH, SEARCH_PAUSE, |page: &mut HistoryPage| {
            page.refresh()
        });
    }

    fn choose(&mut self, id: &str, url: &str, chosen: bool) {
        match chosen {
            true => self.chosen.insert(id.to_owned(), url.to_owned()),
            false => self.chosen.remove(id),
        };
    }

    fn forget(&mut self, urls: Vec<String>) {
        let Some(history) = self.history.clone() else {
            return;
        };
        let visible: Vec<Item> = self
            .items
            .iter()
            .filter(|item| match item {
                Item::Visit(entry) => !urls.contains(&entry.url),
                Item::Day(_) => true,
            })
            .cloned()
            .collect();
        let items = visible
            .iter()
            .enumerate()
            .filter(|(at, item)| match item {
                Item::Day(_) => matches!(visible.get(at + 1), Some(Item::Visit(_))),
                Item::Visit(_) => true,
            })
            .map(|(_, item)| item.clone())
            .collect();
        self.items.set(Arc::new(items));
        self.chosen.retain(|_, url| !urls.contains(url));
        spawn_keyed(
            (FORGET, urls.clone()),
            async move {
                for url in &urls {
                    history.delete_url(url).await?;
                }
                Ok(())
            },
            |_: &mut HistoryPage, forgotten: rikka_web::Result<()>| {
                if let Err(error) = forgotten {
                    tracing::warn!(%error, "failed to remove history entries");
                }
            },
        );
    }

    fn forget_chosen(&mut self) {
        let urls: Vec<String> = self.chosen.values().cloned().collect();
        self.forget(urls);
    }

    pub(crate) fn clear(&mut self, span: Span) {
        let Some(history) = self.history.clone() else {
            return;
        };
        let cleared = match span.length() {
            None => Either::Left(history.delete_all()),
            Some(_) => Either::Right(history.delete_range(span.since(), SystemTime::now())),
        };
        spawn(
            cleared,
            |page: &mut HistoryPage, cleared: rikka_web::Result<()>| {
                if let Err(error) = cleared {
                    tracing::warn!(%error, "failed to clear history");
                }
                page.refresh();
            },
        );
    }
}

impl std::fmt::Debug for HistoryPage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HistoryPage")
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

pub(crate) fn view(page: &HistoryPage) -> Element {
    let chosen = page.chosen.len();
    let bar = row! {
        text_field(&page.search)
            .placeholder(tr!("browser.history.search"))
            .on_change(|page: &mut HistoryPage| page.searched())
            .grow(1.0)
            .min_width(0.0);
        if chosen > 0 {
            button(text(tr!("browser.history.remove-chosen", count = &chosen.to_string())))
                .secondary()
                .small()
                .on_press(|page: &mut HistoryPage| page.forget_chosen());
        }
        button(text(tr!("browser.history.clear")))
            .secondary()
            .small()
            .on_press(|menus: &mut Menus| menus.open_below(Subject::ClearHistory, CLEAR_BUTTON))
            .build()
            .keyed(CLEAR_BUTTON);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width();
    let body = match (*page.status, page.items.is_empty()) {
        (Status::Failed, _) => {
            status_page(MaterialSymbol::WARNING, tr!("browser.history.unreadable")).any()
        }
        (Status::Waiting, true) => container(spinner().extent(Length::px(20.0)))
            .center()
            .fill()
            .any(),
        (Status::Ready, true) => {
            status_page(MaterialSymbol::HISTORY, tr!("browser.history.empty")).any()
        }
        (_, false) => list(page),
    };
    container(
        column! {
            text(tr!("browser.history.title")).title();
            bar;
            body;
        }
        .gap(Space::NORMAL)
        .padding(Inset::all(Space::WIDE))
        .max_width(WIDEST)
        .fill(),
    )
    .align_x(Align::Center)
    .fill()
    .any()
}

fn list(page: &HistoryPage) -> Element {
    let items = Arc::clone(&page.items);
    let chosen = (*page.chosen).clone();
    let count = items.len();
    lazy_column(count, move |slot| match &items[slot.index] {
        Item::Day(day) => container(text(day.to_string()).label().semibold())
            .padding(Inset::edges(
                Space::NORMAL,
                Space::SNUG,
                Space::TIGHT,
                Space::SNUG,
            ))
            .align_y(Align::End)
            .height(HEADER)
            .fill_width()
            .any(),
        Item::Visit(entry) => visit(entry, chosen.contains_key(&entry.id)),
    })
    .on_visible(|page: &mut HistoryPage, shown: Range<usize>| page.showing(shown))
    .grow(1.0)
    .fill_width()
    .any()
}

fn visit(entry: &Arc<HistoryItem>, chosen: bool) -> Element {
    let time = entry
        .last_visit_time
        .and_then(|at| days::local_time(Timestamp::from(at)))
        .map(|local_time| local_time.time)
        .unwrap_or_default();
    let title = crate::sidebar::titled(&entry.title, &entry.url);
    let host = crate::address::display_address(&entry.url).host;
    let (id, url) = (entry.id.clone(), entry.url.clone());
    let (opened, opened_aside, forgotten) = (url.clone(), url.clone(), url.clone());
    container(
        row! {
            checkbox(chosen).on_toggle(move |page: &mut HistoryPage, on: bool| {
                page.choose(&id, &url, on);
            });
            text(time).caption().muted().no_wrap().width(44.0);
            column! {
                text(title).label().no_wrap().min_width(0.0);
                text(host).caption().muted().no_wrap().min_width(0.0);
            }
            .grow(1.0)
            .min_width(0.0);
            glyph_button(MaterialSymbol::OPEN_IN_NEW, 16.0)
                .on_press(move |shell: &mut Shell| shell.open_aside(opened_aside.clone()));
            glyph_button(MaterialSymbol::CLOSE, 16.0)
                .on_press(move |page: &mut HistoryPage| page.forget(vec![forgotten.clone()]));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .padding(Inset::xy(Space::SNUG, Space::NONE))
        .fill(),
    )
    .radius(Corner::Item)
    .on_press(move |shell: &mut Shell| shell.go(opened.clone()))
    .height(ROW)
    .fill_width()
    .any()
}
