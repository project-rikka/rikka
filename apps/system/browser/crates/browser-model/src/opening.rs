use browser_store::{Restored, TabRecord, Timestamp};

use crate::page::{LivePage, Page};
use crate::{Closed, Home, Model, ProfileKey, Reopening, Saved, Slot, TabId, WindowId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Top,
    After(TabId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Disposal {
    Archive { remembered: bool },
    Discard,
}

impl<L: LivePage> Model<L> {
    pub fn open_tab(
        &mut self,
        window: WindowId,
        address: Option<String>,
        placement: Placement,
        focus: bool,
    ) -> Option<TabId> {
        let saved = Saved::new(address.unwrap_or_default(), "");
        self.place_new(window, saved, Page::Unloaded, placement, focus)
    }

    pub fn adopt_tab(
        &mut self,
        window: WindowId,
        live: L,
        address: String,
        placement: Placement,
        focus: bool,
    ) -> Option<TabId> {
        let saved = Saved::new(address, "");
        self.place_new(window, saved, Page::Live(live), placement, focus)
    }

    fn place_new(
        &mut self,
        window: WindowId,
        saved: Saved,
        page: Page<L>,
        placement: Placement,
        focus: bool,
    ) -> Option<TabId> {
        let layout = self.windows.get(window)?;
        let space = layout.space();
        let at = match placement {
            Placement::After(opener) => layout
                .place_in_today(opener)
                .filter(|(candidate, _)| *candidate == space)
                .map_or(0, |(_, at)| at + 1),
            Placement::Top => 0,
        };
        let profile = self.profile_here(window);
        let id = self.insert_tab(profile, Home::Today(space), saved);
        if let Some(tab) = self.tabs.get_mut(id) {
            tab.set_page(page);
        }
        let layout = self.windows.get_mut(window)?;
        layout.insert_today(space, at, id);
        if focus {
            layout.set_active(Some(id));
        }
        Some(id)
    }

    pub fn close_tab(&mut self, window: WindowId, tab: TabId) -> Option<L::View> {
        self.close_with(window, tab, Disposal::Archive { remembered: true })
    }

    fn close_with(&mut self, window: WindowId, tab: TabId, disposal: Disposal) -> Option<L::View> {
        let layout = self.windows.get(window)?;
        let was_active = layout.active() == Some(tab);
        let neighbour = self.neighbour(window, tab);
        let private = layout.private;
        let layout = self.windows.get_mut(window)?;
        if !layout.take(tab) {
            return None;
        }
        let closed = self.tabs.remove(tab)?;
        if was_active {
            self.fall_back(window, neighbour);
        }
        let saved = closed.saved();
        let disposal = match closed.is_blank() {
            true => Disposal::Discard,
            false => disposal,
        };
        match (closed.home, disposal, private) {
            (Home::Today(space), Disposal::Archive { .. }, true) => {
                self.windows
                    .get_mut(window)?
                    .remember_closed(Closed { tab, space, saved });
            }
            (Home::Today(Some(space)), Disposal::Archive { remembered }, false) => {
                let record = TabRecord {
                    url: saved.url,
                    title: saved.title,
                };
                if remembered {
                    self.store.push_closed_tab(space, record.clone());
                }
                self.store.archive_tabs(space, vec![record]);
            }
            _ => {}
        }
        closed.page().view().cloned()
    }

    pub fn close_below(&mut self, window: WindowId, tab: TabId) -> Vec<L::View> {
        let below: Vec<TabId> = self
            .windows
            .get(window)
            .map(|layout| {
                layout
                    .today_here()
                    .iter()
                    .skip_while(|candidate| **candidate != tab)
                    .skip(1)
                    .copied()
                    .collect()
            })
            .unwrap_or_default();
        below
            .into_iter()
            .filter_map(|below| self.close_tab(window, below))
            .collect()
    }

    pub fn clear_today(&mut self, window: WindowId) -> Vec<L::View> {
        let today = self
            .windows
            .get(window)
            .map(|layout| layout.today_here().to_vec())
            .unwrap_or_default();
        today
            .into_iter()
            .filter_map(|tab| self.close_tab(window, tab))
            .collect()
    }

    pub fn archive_idle(&mut self, window: WindowId, before: Timestamp) -> Vec<L::View> {
        let Some(layout) = self.windows.get(window).filter(|layout| !layout.private) else {
            return Vec::new();
        };
        let active = layout.active();
        let idle: Vec<TabId> = layout
            .todays()
            .flat_map(|(_, tabs)| tabs.iter().copied())
            .filter(|tab| Some(*tab) != active)
            .filter(|tab| {
                self.tabs.get(*tab).is_some_and(|tab_entry| {
                    tab_entry.last_active < before && *tab_entry.meta.sound == crate::Sound::Silent
                })
            })
            .collect();
        let disposal = Disposal::Archive { remembered: false };
        idle.into_iter()
            .filter_map(|tab| self.close_with(window, tab, disposal))
            .collect()
    }

    pub fn close_without_archive(&mut self, window: WindowId, tab: TabId) -> Option<L::View> {
        self.close_with(window, tab, Disposal::Discard)
    }

    pub fn reopen_private(&mut self, window: WindowId) -> Option<TabId> {
        let layout = self.windows.get_mut(window)?;
        if !layout.private {
            return None;
        }
        let closed = layout.take_closed()?;
        let id = self.insert_tab(ProfileKey::Private, Home::Today(None), closed.saved);
        let layout = self.windows.get_mut(window)?;
        layout.insert_today(None, 0, id);
        layout.set_active(Some(id));
        Some(id)
    }

    pub fn reopen_restored(&mut self, window: WindowId, restored: Restored) -> Option<TabId> {
        let layout = self.windows.get(window).filter(|layout| !layout.private)?;
        let space = Some(restored.space)
            .filter(|space| self.spaces.get(*space).is_some())
            .or(layout.space())?;
        let profile = self.spaces.get(space)?.profile;
        let saved = Saved::new(restored.tab.url, restored.tab.title);
        let id = self.insert_tab(
            ProfileKey::Persistent(profile),
            Home::Today(Some(space)),
            saved,
        );
        let layout = self.windows.get_mut(window)?;
        layout.set_space(Some(space));
        layout.insert_today(Some(space), 0, id);
        layout.set_active(Some(id));
        Some(id)
    }

    pub fn reopen_window(&mut self, window: WindowId, reopening: &Reopening) {
        let Some(private) = self.windows.get(window).map(|layout| layout.private) else {
            return;
        };
        if private {
            return;
        }
        for space in &reopening.spaces {
            let Some(profile) = self
                .spaces
                .get(space.space)
                .map(|space_entry| space_entry.profile)
            else {
                continue;
            };
            for tab in &space.tabs {
                let unopened = |model: &Model<L>, slot: Slot| {
                    model
                        .windows
                        .get(window)
                        .is_some_and(|layout| layout.tab_in_slot(slot).is_none())
                };
                let slot = tab
                    .pinned
                    .filter(|pin| self.pinned.pin(*pin).is_some())
                    .map(Slot::Pinned)
                    .or_else(|| {
                        tab.favorite
                            .filter(|favorite| self.favorites.find(*favorite).is_some())
                            .map(Slot::Favorite)
                    })
                    .filter(|slot| unopened(self, *slot));
                let home = match slot {
                    Some(Slot::Pinned(pin)) => Home::Pinned(pin),
                    Some(Slot::Favorite(favorite)) => Home::Favorite(favorite),
                    None => Home::Today(Some(space.space)),
                };
                let mut tab_entry = crate::Tab::new(
                    tab.id,
                    ProfileKey::Persistent(profile),
                    home,
                    tab.saved.clone(),
                );
                tab_entry.last_active = tab.last_active;
                self.tabs.insert(tab_entry);
                let Some(layout) = self.windows.get_mut(window) else {
                    return;
                };
                match slot {
                    Some(slot) => layout.open_slot(slot, tab.id),
                    None => {
                        let at = layout.today(Some(space.space)).len();
                        layout.insert_today(Some(space.space), at, tab.id);
                    }
                }
            }
            if let Some(active) = space.active
                && let Some(layout) = self.windows.get_mut(window)
                && layout.holds(active)
            {
                layout.remember_recent(Some(space.space), active);
            }
        }
        let shown = reopening
            .space
            .filter(|space| self.spaces.get(*space).is_some())
            .or_else(|| self.windows.get(window)?.space());
        let Some(layout) = self.windows.get_mut(window) else {
            return;
        };
        layout.set_space(shown);
        layout.geometry = reopening.geometry;
        let active = layout
            .recent(shown)
            .filter(|tab| layout.holds(*tab))
            .or_else(|| layout.today(shown).first().copied());
        layout.set_active(active);
        let members: Vec<_> = reopening
            .spaces
            .iter()
            .flat_map(|space| &space.tabs)
            .filter_map(|tab| Some((tab.id, tab.split?)))
            .collect();
        self.restore_splits(window, &members, &reopening.splits);
    }

    pub fn select(&mut self, window: WindowId, tab: TabId) {
        let Some(home) = self.tabs.get(tab).map(|tab_entry| tab_entry.home) else {
            return;
        };
        let Some(layout) = self.windows.get_mut(window) else {
            return;
        };
        if !layout.holds(tab) {
            return;
        }
        if let Home::Today(space) = home
            && layout.space() != space
        {
            layout.set_space(space);
        }
        layout.set_active(Some(tab));
    }

    pub fn select_slot(&mut self, window: WindowId, slot: Slot) -> Option<TabId> {
        if let Some(opened) = self.windows.get(window)?.tab_in_slot(slot) {
            self.select(window, opened);
            return Some(opened);
        }
        let (profile, home, saved) = match slot {
            Slot::Pinned(pin) => {
                let (space, pin) = self.pinned.pin(pin)?;
                let profile = self.spaces.get(space)?.profile;
                (profile, Home::Pinned(pin.id), pin.saved.clone())
            }
            Slot::Favorite(favorite) => {
                let (profile, favorite) = self.favorites.find(favorite)?;
                (profile, Home::Favorite(favorite.id), favorite.saved.clone())
            }
        };
        let id = self.insert_tab(ProfileKey::Persistent(profile), home, saved);
        let layout = self.windows.get_mut(window)?;
        layout.open_slot(slot, id);
        layout.set_active(Some(id));
        Some(id)
    }

    pub fn cycle(&mut self, window: WindowId, by: isize) {
        let strip = self.strip(window);
        if strip.is_empty() {
            return;
        }
        let active = self.windows.get(window).and_then(|layout| layout.active());
        let at = active
            .and_then(|active| strip.iter().position(|tab| *tab == active))
            .map_or(0, |at| {
                (at as isize + by).rem_euclid(strip.len() as isize) as usize
            });
        self.select(window, strip[at]);
    }

    pub fn select_nth(&mut self, window: WindowId, nth: usize) {
        if let Some(tab) = self.strip(window).get(nth).copied() {
            self.select(window, tab);
        }
    }

    pub fn select_last(&mut self, window: WindowId) {
        if let Some(tab) = self.strip(window).last().copied() {
            self.select(window, tab);
        }
    }

    pub fn duplicate(&mut self, window: WindowId, tab: TabId) -> Option<TabId> {
        let address = self.tabs.get(tab)?.address();
        let placement = match self.windows.get(window)?.place_in_today(tab) {
            Some(_) => Placement::After(tab),
            None => Placement::Top,
        };
        self.open_tab(window, Some(address), placement, true)
    }

    fn neighbour(&self, window: WindowId, tab: TabId) -> Option<TabId> {
        let layout = self.windows.get(window)?;
        let today = layout.today_here();
        if let Some(at) = today.iter().position(|candidate| *candidate == tab) {
            return today
                .get(at + 1)
                .or_else(|| at.checked_sub(1).and_then(|before| today.get(before)))
                .copied();
        }
        let strip = self.strip(window);
        let at = strip.iter().position(|candidate| *candidate == tab)?;
        strip
            .get(at + 1)
            .or_else(|| at.checked_sub(1).and_then(|before| strip.get(before)))
            .copied()
    }

    pub fn fall_back(&mut self, window: WindowId, neighbour: Option<TabId>) {
        let Some(layout) = self.windows.get(window) else {
            return;
        };
        let space = layout.space();
        let next = neighbour
            .filter(|tab| layout.holds(*tab))
            .or_else(|| layout.recent(space).filter(|tab| layout.holds(*tab)))
            .or_else(|| layout.today_here().first().copied());
        if let Some(layout) = self.windows.get_mut(window) {
            layout.set_active(next);
        }
    }
}
