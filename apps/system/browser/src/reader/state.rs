use std::collections::HashSet;
use std::sync::Arc;

use browser_reader::Article;
use rikka_app::prelude::*;
use rikka_web::Session;

use browser_model::TabId;

#[derive(Clone, Debug)]
pub(crate) struct Reading {
    pub(crate) tab: TabId,
    pub(crate) view: Session,
    pub(crate) article: Arc<Article>,
    pub(crate) document: Arc<str>,
    pub(crate) dark: bool,
}

impl PartialEq for Reading {
    fn eq(&self, other: &Reading) -> bool {
        self.tab == other.tab
            && self.view == other.view
            && Arc::ptr_eq(&self.article, &other.article)
            && Arc::ptr_eq(&self.document, &other.document)
    }
}

#[derive(State)]
pub(crate) struct Reader {
    offered: Tracked<HashSet<TabId>>,
    open: Tracked<Option<Reading>>,
}

impl Reader {
    pub(crate) fn new() -> Reader {
        Reader {
            offered: Tracked::new(HashSet::new()),
            open: Tracked::new(None),
        }
    }

    pub(crate) fn offers(&self, tab: TabId) -> bool {
        self.offered.contains(&tab)
    }

    pub(crate) fn reads(&self, tab: TabId) -> bool {
        self.open.as_ref().is_some_and(|reading| reading.tab == tab)
    }

    pub(crate) fn offer(&mut self, tab: TabId, readable: bool) {
        if self.offered.contains(&tab) == readable {
            return;
        }
        match readable {
            true => self.offered.insert(tab),
            false => self.offered.remove(&tab),
        };
    }

    pub(crate) fn reading(&self) -> Option<&Reading> {
        self.open.as_ref()
    }

    pub(crate) fn show(&mut self, reading: Reading) {
        self.open.set(Some(reading));
    }

    pub(crate) fn restyle(&mut self, document: Arc<str>) {
        if let Some(reading) = self.open.as_ref() {
            let reading = Reading {
                document,
                ..reading.clone()
            };
            self.open.set(Some(reading));
        }
    }

    pub(crate) fn close(&mut self) -> Option<Reading> {
        if self.open.is_none() {
            return None;
        }
        self.open.take()
    }

    pub(crate) fn retain(&mut self, alive: impl Fn(TabId) -> bool) {
        if self.offered.iter().all(|tab| alive(*tab)) {
            return;
        }
        self.offered.retain(|tab| alive(*tab));
    }

    pub(crate) fn forget(&mut self, tab: TabId) -> Option<Reading> {
        self.offer(tab, false);
        match self.reads(tab) {
            true => self.close(),
            false => None,
        }
    }
}

impl std::fmt::Debug for Reader {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Reader")
            .field("offered", &self.offered.len())
            .field("open", &self.open.as_ref().map(|reading| reading.tab))
            .finish()
    }
}
