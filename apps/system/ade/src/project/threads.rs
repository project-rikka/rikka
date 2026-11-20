use std::collections::HashMap;

use rikka_app::prelude::*;
use rikka_thread::{Store, Summary, Thread};

#[derive(Debug, Default, State)]
pub(crate) struct Threads {
    listed: Tracked<Vec<Summary>>,
    loaded: Tracked<HashMap<String, Thread>>,
}

impl Threads {
    #[must_use]
    pub(crate) fn listed(&self) -> &[Summary] {
        &self.listed
    }

    #[must_use]
    pub(crate) fn first(&self) -> String {
        self.listed
            .first()
            .map(Summary::id)
            .unwrap_or_default()
            .to_owned()
    }

    #[must_use]
    pub(crate) fn summary(&self, id: &str) -> Option<&Summary> {
        self.listed.iter().find(|summary| summary.id() == id)
    }

    #[must_use]
    pub(crate) fn get(&self, id: &str) -> Option<&Thread> {
        self.loaded.get(id)
    }

    #[must_use]
    pub(crate) fn get_mut(&mut self, id: &str) -> Option<&mut Thread> {
        self.loaded.get_mut(id)
    }

    #[must_use]
    pub(crate) fn holds(&self, id: &str) -> bool {
        self.loaded.contains_key(id)
    }

    #[must_use]
    pub(crate) fn is_listed(&self, id: &str) -> bool {
        self.listed.iter().any(|summary| summary.id() == id)
    }

    pub(crate) fn loaded(&self) -> impl Iterator<Item = &Thread> {
        self.loaded.values()
    }

    pub(crate) fn recall(&mut self, listed: Vec<Summary>) {
        let known: Vec<Summary> = listed
            .into_iter()
            .filter(|summary| !self.is_listed(summary.id()))
            .collect();
        self.listed.splice(..0, known);
        self.listed
            .sort_by_key(|summary| std::cmp::Reverse(summary.updated()));
    }

    pub(crate) fn begin(&mut self, thread: Thread) {
        self.listed.insert(0, thread.summary());
        self.loaded.insert(thread.id().to_owned(), thread);
    }

    pub(crate) fn opened(&mut self, thread: Thread) {
        if !self.is_listed(thread.id()) {
            return;
        }
        self.loaded.entry(thread.id().to_owned()).or_insert(thread);
    }

    pub(crate) fn take(&mut self, id: &str) -> Option<Thread> {
        self.loaded.remove(id)
    }

    pub(crate) fn forget(&mut self, id: &str) {
        self.listed.retain(|summary| summary.id() != id);
        self.loaded.remove(id);
    }

    pub(crate) fn keep(&mut self, id: &str, store: Option<&Store>) {
        let Some(thread) = self.loaded.get(id) else {
            return;
        };
        if let Some(store) = store {
            store.keep(thread);
        }
        let summary = thread.summary();
        self.list(summary);
    }

    pub(crate) fn list(&mut self, summary: Summary) {
        match self
            .listed
            .iter()
            .position(|listed| listed.id() == summary.id())
        {
            Some(at) if self.listed[at] == summary => {}
            Some(at) => self.listed[at] = summary,
            None => self.listed.insert(0, summary),
        }
    }

    pub(crate) fn release_unless(&mut self, keep: impl Fn(&str) -> bool) {
        let removed: Vec<String> = self
            .loaded
            .keys()
            .filter(|id| !keep(id) && self.is_listed(id))
            .cloned()
            .collect();
        if removed.is_empty() {
            return;
        }
        self.loaded.retain(|id, _| !removed.contains(id));
    }
}
