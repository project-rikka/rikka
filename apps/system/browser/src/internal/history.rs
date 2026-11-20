use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use browser_pages::Cause;
use browser_store::{HistoryCursor, HistoryEntry, HistoryQuery, ProfileId, Store, VisitId};
use jiff::civil::Date;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::internal::days;
use crate::internal::status::Status;
use crate::menus::{Menus, Subject};
use crate::shell::Shell;
use crate::widgets::glyph_button;

const PAGE: usize = 100;
const AHEAD: usize = 30;
const ROW: f32 = 44.0;
const HEADER: f32 = 36.0;
const WIDEST: f32 = 860.0;
const FETCH: &str = "rikka-browser-history";
const SEARCH: &str = "rikka-browser-history-search";
const SEARCH_PAUSE: Duration = Duration::from_millis(150);
pub(crate) const CLEAR_BUTTON: &str = "rikka-browser-history-clear";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    Day(Arc<str>),
    Visit(Arc<HistoryEntry>),
}

#[derive(State)]
pub(crate) struct HistoryPage {
    #[untracked]
    store: Option<Store>,
    #[untracked]
    profile: Option<ProfileId>,
    #[untracked]
    search: TextDocument,
    #[untracked]
    cursor: Option<HistoryCursor>,
    #[untracked]
    last_day: Option<Date>,
    #[untracked]
    waiting: bool,
    #[untracked]
    exhausted: bool,
    items: Tracked<Arc<Vec<Item>>>,
    chosen: Tracked<BTreeMap<VisitId, String>>,
    status: Tracked<Status>,
}

impl HistoryPage {
    pub(crate) fn new() -> HistoryPage {
        HistoryPage {
            store: None,
            profile: None,
            search: TextDocument::new(""),
            cursor: None,
            last_day: None,
            waiting: false,
            exhausted: false,
            items: Tracked::new(Arc::new(Vec::new())),
            chosen: Tracked::new(BTreeMap::new()),
            status: Tracked::new(Status::Waiting),
        }
    }

    pub(crate) fn open(&mut self, store: Store, profile: Option<ProfileId>) {
        self.store = Some(store);
        self.profile = profile;
        self.refresh();
    }

    pub(crate) fn profile(&self) -> Option<ProfileId> {
        self.profile
    }

    pub(crate) fn items(&self) -> &Arc<Vec<Item>> {
        &self.items
    }

    pub(crate) fn status(&self) -> Status {
        *self.status
    }

    pub(crate) fn chosen(&self) -> &BTreeMap<VisitId, String> {
        &self.chosen
    }

    pub(crate) fn refresh(&mut self) {
        self.cursor = None;
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
        let (Some(store), Some(profile)) = (self.store.clone(), self.profile) else {
            cancel(FETCH);
            self.status.set(Status::Ready);
            return;
        };
        if self.waiting || self.exhausted {
            return;
        }
        self.waiting = true;
        let mut query = HistoryQuery::new(PAGE).text(self.search.text());
        if let Some(cursor) = self.cursor {
            query = query.after(cursor);
        }
        spawn_blocking_keyed(
            FETCH,
            move || {
                if let Err(error) = store.flush() {
                    tracing::warn!(%error, "failed to save browsing data");
                }
                store.history(profile, &query)
            },
            move |page: &mut HistoryPage, found: browser_store::Result<Vec<HistoryEntry>>| {
                page.loaded(found);
            },
        );
    }

    fn loaded(&mut self, found: browser_store::Result<Vec<HistoryEntry>>) {
        self.waiting = false;
        let entries = match found {
            Ok(entries) => entries,
            Err(error) => {
                tracing::warn!(%error, "failed to read history");
                self.status.set(Status::Failed);
                return;
            }
        };
        self.exhausted = entries.len() < PAGE;
        self.cursor = entries.last().map(HistoryEntry::cursor).or(self.cursor);
        let today = days::today();
        let mut items = Vec::clone(&self.items);
        for entry in entries {
            let date = days::local_time(entry.visited_at).map(|local_time| local_time.date);
            if date.is_some() && date != self.last_day {
                self.last_day = date;
                if let Some(date) = date {
                    items.push(Item::Day(days::day(date, today).into()));
                }
            }
            items.push(Item::Visit(Arc::new(entry)));
        }
        self.items.set(Arc::new(items));
        self.status.set(Status::Ready);
    }

    pub(crate) fn showing(&mut self, shown: Range<usize>) {
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

    fn choose(&mut self, visit: VisitId, url: &str, chosen: bool) {
        match chosen {
            true => self.chosen.insert(visit, url.to_owned()),
            false => self.chosen.remove(&visit),
        };
    }

    pub(crate) fn forget(&mut self, urls: &[String]) {
        let Some((store, profile)) = self.store.clone().zip(self.profile) else {
            return;
        };
        for url in urls {
            store.delete_url(profile, url.clone());
        }
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
    }

    fn forget_chosen(&mut self) {
        let urls: Vec<String> = self.chosen.values().cloned().collect();
        self.forget(&urls);
    }

    pub(crate) fn clear(&mut self, range: browser_store::TimeRange) {
        let Some((store, profile)) = self.store.clone().zip(self.profile) else {
            return;
        };
        store.delete_range(profile, range);
        self.refresh();
    }
}

impl std::fmt::Debug for HistoryPage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HistoryPage")
            .field("profile", &self.profile)
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

pub(crate) fn view(page: &HistoryPage) -> Element {
    if page.profile().is_none() {
        return status_page(MaterialSymbol::HISTORY, tr!("browser.history.private")).any();
    }
    let chosen = page.chosen().len();
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
    let body = match (page.status(), page.items().is_empty()) {
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
    let items = Arc::clone(page.items());
    let chosen = page.chosen().clone();
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
        Item::Visit(entry) => visit(entry, chosen.contains_key(&entry.visit)),
    })
    .on_visible(|page: &mut HistoryPage, shown: Range<usize>| page.showing(shown))
    .grow(1.0)
    .fill_width()
    .any()
}

fn visit(entry: &Arc<HistoryEntry>, chosen: bool) -> Element {
    let time = days::local_time(entry.visited_at)
        .map(|local_time| local_time.time)
        .unwrap_or_default();
    let title = crate::sidebar::titled(&entry.title, &entry.url);
    let host = crate::address::display_address(&entry.url).host;
    let (visit, url) = (entry.visit, entry.url.clone());
    let (opened, opened_aside, forgotten) = (url.clone(), url.clone(), url.clone());
    container(
        row! {
            checkbox(chosen).on_toggle(move |page: &mut HistoryPage, on: bool| {
                page.choose(visit, &url, on);
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
                .on_press(move |page: &mut HistoryPage| page.forget(std::slice::from_ref(&forgotten)));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .padding(Inset::xy(Space::SNUG, Space::NONE))
        .fill(),
    )
    .radius(Corner::Item)
    .on_press(move |shell: &mut Shell| shell.go(opened.clone(), Cause::Link))
    .height(ROW)
    .fill_width()
    .any()
}
