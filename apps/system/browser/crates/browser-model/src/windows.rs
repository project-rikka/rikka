use std::collections::HashMap;

use rikka_ui::{Observers, Tracked};

use crate::closed::Closed;
use crate::ids::{FavoriteId, PinnedTabId, SpaceId, TabId, WindowId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    Pinned(PinnedTabId),
    Favorite(FavoriteId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

#[derive(Debug)]
pub struct Layout {
    pub id: WindowId,
    pub private: bool,
    pub little: bool,
    pub geometry: Option<Geometry>,
    space: Tracked<Option<SpaceId>>,
    active: Tracked<Option<TabId>>,
    today: HashMap<Option<SpaceId>, Vec<TabId>>,
    slot_tabs: HashMap<Slot, TabId>,
    recent: HashMap<Option<SpaceId>, TabId>,
    closed: Vec<Closed>,
    observers: Observers,
}

impl Layout {
    pub fn new(id: WindowId, private: bool, space: Option<SpaceId>) -> Layout {
        Layout {
            id,
            private,
            little: false,
            geometry: None,
            space: Tracked::new(space),
            active: Tracked::new(None),
            today: HashMap::new(),
            slot_tabs: HashMap::new(),
            recent: HashMap::new(),
            closed: Vec::new(),
            observers: Observers::new(),
        }
    }

    pub fn space(&self) -> Option<SpaceId> {
        *self.space
    }

    pub fn active(&self) -> Option<TabId> {
        *self.active
    }

    pub fn today(&self, space: Option<SpaceId>) -> &[TabId] {
        self.observers.read();
        self.today.get(&space).map_or(&[], Vec::as_slice)
    }

    pub fn today_here(&self) -> &[TabId] {
        self.today(self.space())
    }

    pub fn todays(&self) -> impl Iterator<Item = (Option<SpaceId>, &[TabId])> {
        self.today
            .iter()
            .map(|(space, tabs)| (*space, tabs.as_slice()))
    }

    pub fn tab_in_slot(&self, slot: Slot) -> Option<TabId> {
        self.observers.read();
        self.slot_tabs.get(&slot).copied()
    }

    pub fn tabs(&self) -> Vec<TabId> {
        self.observers.read();
        self.slot_tabs
            .values()
            .copied()
            .chain(self.today.values().flatten().copied())
            .collect()
    }

    pub fn holds(&self, tab: TabId) -> bool {
        self.slot_tabs.values().any(|candidate| *candidate == tab)
            || self
                .today
                .values()
                .flatten()
                .any(|candidate| *candidate == tab)
    }

    pub fn place_in_today(&self, tab: TabId) -> Option<(Option<SpaceId>, usize)> {
        self.today.iter().find_map(|(space, tabs)| {
            tabs.iter()
                .position(|candidate| *candidate == tab)
                .map(|at| (*space, at))
        })
    }

    pub fn slot_of(&self, tab: TabId) -> Option<Slot> {
        self.slot_tabs
            .iter()
            .find_map(|(slot, candidate)| (*candidate == tab).then_some(*slot))
    }

    pub fn recent(&self, space: Option<SpaceId>) -> Option<TabId> {
        self.recent.get(&space).copied()
    }

    pub fn remember_recent(&mut self, space: Option<SpaceId>, tab: TabId) {
        self.recent.insert(space, tab);
    }

    pub fn set_space(&mut self, space: Option<SpaceId>) {
        self.space.set(space);
    }

    pub fn set_active(&mut self, tab: Option<TabId>) {
        if let Some(tab) = tab {
            self.recent.insert(self.space(), tab);
        }
        self.active.set(tab);
    }

    pub fn insert_today(&mut self, space: Option<SpaceId>, at: usize, tab: TabId) {
        let tabs = self.today.entry(space).or_default();
        tabs.insert(at.min(tabs.len()), tab);
        self.observers.changed();
    }

    pub fn reorder_today(&mut self, space: Option<SpaceId>, order: Vec<TabId>) {
        let Some(tabs) = self.today.get_mut(&space) else {
            return;
        };
        let mut sorted = order.clone();
        sorted.sort_unstable();
        let mut current = tabs.clone();
        current.sort_unstable();
        if sorted == current && *tabs != order {
            *tabs = order;
            self.observers.changed();
        }
    }

    pub fn take(&mut self, tab: TabId) -> bool {
        let removed = match (self.place_in_today(tab), self.slot_of(tab)) {
            (Some((space, at)), _) => {
                self.today.entry(space).or_default().remove(at);
                true
            }
            (None, Some(slot)) => self.slot_tabs.remove(&slot).is_some(),
            (None, None) => false,
        };
        if removed {
            self.recent.retain(|_, opened_tab| *opened_tab != tab);
            if self.active() == Some(tab) {
                self.active.set(None);
            }
            self.observers.changed();
        }
        removed
    }

    pub fn open_slot(&mut self, slot: Slot, tab: TabId) {
        self.slot_tabs.insert(slot, tab);
        self.observers.changed();
    }

    pub fn move_slot(&mut self, from: Slot, to: Slot) {
        if let Some(tab) = self.slot_tabs.remove(&from) {
            self.slot_tabs.insert(to, tab);
            self.observers.changed();
        }
    }

    pub fn remember_closed(&mut self, closed: Closed) {
        self.closed.push(closed);
    }

    pub fn take_closed(&mut self) -> Option<Closed> {
        self.closed.pop()
    }
}

#[derive(Debug, Default)]
pub struct Windows {
    list: Vec<Layout>,
    focus: Vec<WindowId>,
}

impl Windows {
    pub fn get(&self, id: WindowId) -> Option<&Layout> {
        self.list.iter().find(|layout| layout.id == id)
    }

    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut Layout> {
        self.list.iter_mut().find(|layout| layout.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Layout> {
        self.list.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Layout> {
        self.list.iter_mut()
    }

    pub fn add(&mut self, layout: Layout) {
        self.focus.push(layout.id);
        self.list.push(layout);
    }

    pub fn remove(&mut self, id: WindowId) -> Option<Layout> {
        self.focus.retain(|focused| *focused != id);
        let at = self.list.iter().position(|layout| layout.id == id)?;
        Some(self.list.remove(at))
    }

    pub fn window_of(&self, tab: TabId) -> Option<WindowId> {
        self.list
            .iter()
            .find(|layout| layout.holds(tab))
            .map(|layout| layout.id)
    }

    pub fn focused(&mut self, id: WindowId) {
        self.focus.retain(|focused| *focused != id);
        self.focus.push(id);
    }

    pub fn main(&self) -> Option<WindowId> {
        self.focus.iter().rev().copied().find(|id| {
            self.get(*id)
                .is_some_and(|layout| !layout.private && !layout.little)
        })
    }
}
