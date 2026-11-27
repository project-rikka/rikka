use std::sync::Arc;
use std::time::{Duration, SystemTime};

use browser_store::{ClosedTab, SpaceId, Store};
use rikka_app::prelude::*;
use rikka_web::{History, HistoryItem, HistoryQuery};

use crate::home::clock::Minute;
use browser_model::TabId;

const CLOCK: &str = "rikka-browser-home-clock";
const SHELF: &str = "rikka-browser-home-shelf";
const SITES: &str = "rikka-browser-home-sites";
const SITES_READ: usize = 24;
const VISITS_READ: u32 = 500;
const VISITS_SPAN: Duration = Duration::from_secs(90 * 24 * 60 * 60);
const CLOSED_SHOWN: usize = 5;

#[derive(State)]
pub(crate) struct HomePage {
    #[untracked]
    shown_in: Option<TabId>,
    #[untracked]
    store: Option<Store>,
    #[untracked]
    history: Option<History>,
    #[untracked]
    spaces: Vec<SpaceId>,
    now: Tracked<Minute>,
    sites: Tracked<Arc<[HistoryItem]>>,
    closed: Tracked<Arc<[ClosedTab]>>,
}

impl HomePage {
    pub(crate) fn new() -> HomePage {
        HomePage {
            shown_in: None,
            store: None,
            history: None,
            spaces: Vec::new(),
            now: Tracked::new(Minute::now().0),
            sites: Tracked::new(Arc::from([])),
            closed: Tracked::new(Arc::from([])),
        }
    }

    pub(crate) fn shown_in(&self) -> Option<TabId> {
        self.shown_in
    }

    pub(crate) fn open(
        &mut self,
        tab: TabId,
        store: Option<Store>,
        history: Option<History>,
        spaces: Vec<SpaceId>,
    ) {
        self.shown_in = Some(tab);
        self.store = store;
        self.history = history;
        self.spaces = spaces;
        self.tick();
        self.refresh();
    }

    pub(crate) fn close(&mut self) {
        self.shown_in = None;
        cancel(CLOCK);
        cancel(SHELF);
        cancel(SITES);
    }

    pub(crate) fn now(&self) -> Minute {
        *self.now
    }

    pub(crate) fn sites(&self) -> &Arc<[HistoryItem]> {
        &self.sites
    }

    pub(crate) fn closed(&self) -> &Arc<[ClosedTab]> {
        &self.closed
    }

    pub(crate) fn site(&self, id: &str) -> Option<&HistoryItem> {
        self.sites.iter().find(|site| site.id == id)
    }

    fn tick(&mut self) {
        let (now, next) = Minute::now();
        self.now.set(now);
        after_keyed(CLOCK, next, |page: &mut HomePage| page.tick());
    }

    fn refresh(&mut self) {
        match self.store.clone() {
            Some(store) => {
                let spaces = self.spaces.clone();
                spawn_blocking_keyed(
                    SHELF,
                    move || closed(&store, &spaces),
                    |page: &mut HomePage, closed: browser_store::Result<Vec<ClosedTab>>| {
                        match closed {
                            Ok(closed) => page.closed.set(Arc::from(closed)),
                            Err(error) => tracing::warn!(%error, "failed to read closed tabs"),
                        }
                    },
                );
            }
            None => self.closed.set(Arc::from([])),
        }
        match self.history.clone() {
            Some(history) => {
                let since = SystemTime::now()
                    .checked_sub(VISITS_SPAN)
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                let query = HistoryQuery::new("").since(since).limit(VISITS_READ);
                spawn_keyed(
                    SITES,
                    history.search(&query),
                    |page: &mut HomePage, visited: rikka_web::Result<Vec<HistoryItem>>| {
                        match visited {
                            Ok(visited) => page.sites.set(Arc::from(top_sites(visited))),
                            Err(error) => tracing::warn!(%error, "failed to read top sites"),
                        }
                    },
                );
            }
            None => self.sites.set(Arc::from([])),
        }
    }
}

fn top_sites(mut visited: Vec<HistoryItem>) -> Vec<HistoryItem> {
    visited.retain(|item| item.url.starts_with("https://") || item.url.starts_with("http://"));
    visited.sort_by(|left, right| {
        right
            .visit_count
            .cmp(&left.visit_count)
            .then(right.last_visit_time.cmp(&left.last_visit_time))
    });
    visited.truncate(SITES_READ);
    visited
}

fn closed(store: &Store, spaces: &[SpaceId]) -> browser_store::Result<Vec<ClosedTab>> {
    store.flush()?;
    Ok(store
        .closed_tabs()?
        .into_iter()
        .filter(|closed| spaces.contains(&closed.space) && !closed.url.is_empty())
        .take(CLOSED_SHOWN)
        .collect())
}

impl std::fmt::Debug for HomePage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HomePage")
            .field("shown_in", &self.shown_in)
            .field("sites", &self.sites.len())
            .field("closed", &self.closed.len())
            .finish_non_exhaustive()
    }
}
