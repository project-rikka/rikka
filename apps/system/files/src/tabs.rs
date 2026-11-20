use files_core::Location;

use crate::tab::Tab;

#[derive(Debug)]
pub(crate) struct Tabs {
    open: Vec<Tab>,
    active: usize,
    next_id: u64,
}

impl Tabs {
    pub(crate) fn new(at: Location) -> Self {
        let mut tabs = Self::empty();
        tabs.open(at, true);
        tabs
    }

    pub(crate) fn with_tab(tab: Tab) -> Self {
        let mut tabs = Self::empty();
        tabs.insert(0, tab);
        tabs
    }

    fn empty() -> Self {
        Self {
            open: Vec::new(),
            active: 0,
            next_id: 0,
        }
    }

    pub(crate) fn open(&mut self, at: Location, show: bool) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let index = (self.active + 1).min(self.open.len());
        self.open.insert(index, Tab::new(id, at));
        if show {
            self.active = index;
        }
        id
    }

    pub(crate) fn close(&mut self, index: usize) -> bool {
        if index >= self.open.len() {
            return true;
        }
        if self.open.len() == 1 {
            return false;
        }
        self.open.remove(index);
        if index < self.active || self.active == self.open.len() {
            self.active -= 1;
        }
        true
    }
    pub(crate) fn take(&mut self, index: usize) -> Option<Tab> {
        if index >= self.open.len() {
            return None;
        }
        let lifted = self.open.remove(index);
        if index < self.active || self.active == self.open.len() {
            self.active = self.active.saturating_sub(1);
        }
        Some(lifted)
    }

    pub(crate) fn insert(&mut self, index: usize, tab: Tab) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let index = index.min(self.open.len());
        self.open.insert(index, tab.adopted(id));
        self.active = index;
        id
    }

    pub(crate) fn select(&mut self, index: usize) {
        if index < self.open.len() {
            self.active = index;
        }
    }

    pub(crate) fn next(&mut self) {
        self.active = (self.active + 1) % self.open.len();
    }

    pub(crate) fn previous(&mut self) {
        self.active = self.active.checked_sub(1).unwrap_or(self.open.len() - 1);
    }

    pub(crate) fn select_nth(&mut self, index: usize) {
        self.active = index.min(self.open.len() - 1);
    }

    pub(crate) fn active(&self) -> &Tab {
        &self.open[self.active]
    }

    pub(crate) fn active_mut(&mut self) -> &mut Tab {
        &mut self.open[self.active]
    }

    pub(crate) fn active_index(&self) -> usize {
        self.active
    }

    pub(crate) fn active_id(&self) -> u64 {
        self.active().id()
    }

    pub(crate) fn by_id(&self, id: u64) -> Option<&Tab> {
        self.open.iter().find(|tab| tab.id() == id)
    }

    pub(crate) fn by_id_mut(&mut self, id: u64) -> Option<&mut Tab> {
        self.open.iter_mut().find(|tab| tab.id() == id)
    }

    pub(crate) fn showing(&self, at: &Location) -> Option<usize> {
        self.open.iter().position(|tab| tab.at() == at)
    }

    pub(crate) fn len(&self) -> usize {
        self.open.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.open.is_empty()
    }
    pub(crate) fn iter(&self) -> std::slice::Iter<'_, Tab> {
        self.open.iter()
    }
}
