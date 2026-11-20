use rikka_app::prelude::Revision;

use crate::tab::Tab;

#[derive(Debug)]
pub(crate) struct Group {
    id: u64,
    tabs: Vec<Tab>,
    active: usize,
    shape: Revision,
}

impl Group {
    pub(crate) fn new(id: u64) -> Self {
        Self::with_tabs(id, Vec::new())
    }

    pub(crate) fn with_tab(id: u64, tab: Tab) -> Self {
        Self::with_tabs(id, vec![tab])
    }

    fn with_tabs(id: u64, tabs: Vec<Tab>) -> Self {
        Self {
            id,
            tabs,
            active: 0,
            shape: Revision::new(),
        }
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn tabs(&self) -> &[Tab] {
        self.shape.read();
        &self.tabs
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.tabs().is_empty()
    }

    pub(crate) fn active_tab(&self) -> Option<&Tab> {
        self.tabs().get(self.active)
    }

    pub(crate) fn active_index(&self) -> usize {
        self.shape.read();
        self.active
    }

    pub(crate) fn position_of(&self, tab: u64) -> Option<usize> {
        self.tabs.iter().position(|candidate| candidate.id() == tab)
    }

    pub(crate) fn tab_mut(&mut self, index: usize) -> Option<&mut Tab> {
        self.tabs.get_mut(index)
    }

    pub(crate) fn select(&mut self, index: usize) {
        if index >= self.tabs.len() || index == self.active {
            return;
        }
        self.clear_preedit();
        self.active = index;
        self.shape.changed();
    }

    pub(crate) fn step(&mut self, forward: bool) {
        let last = self.tabs.len().saturating_sub(1);
        let next = match forward {
            true => (self.active + 1) % self.tabs.len().max(1),
            false if self.active == 0 => last,
            false => self.active - 1,
        };
        self.select(next);
    }

    pub(crate) fn insert(&mut self, index: usize, tab: Tab) {
        self.clear_preedit();
        let index = index.min(self.tabs.len());
        self.tabs.insert(index, tab);
        self.active = index;
        self.shape.changed();
    }

    pub(crate) fn remove(&mut self, index: usize) -> Option<Tab> {
        if index >= self.tabs.len() {
            return None;
        }
        if index == self.active {
            self.clear_preedit();
        }
        let removed = self.tabs.remove(index);
        if index < self.active {
            self.active -= 1;
        }
        self.active = self.active.min(self.tabs.len().saturating_sub(1));
        self.shape.changed();
        Some(removed)
    }

    fn clear_preedit(&self) {
        if let Some(tab) = self.tabs.get(self.active) {
            tab.session().compose("", None);
        }
    }
}
