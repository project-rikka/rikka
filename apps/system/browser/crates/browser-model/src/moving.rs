use crate::model::kept_place;
use crate::page::{LivePage, Page};
use crate::{
    Favorite, FavoriteId, Home, Layout, Model, Node, Pin, PinPlace, PinnedItem, PinnedTabId,
    ProfileKey, Saved, Slot, SpaceId, TabId, WindowId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Row {
    Tab(TabId),
    Pin(PinnedItem),
    Favorite(FavoriteId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Today(usize),
    Pinned(PinPlace),
    Favorites(usize),
}

impl<L: LivePage> Model<L> {
    pub fn drop_row(&mut self, window: WindowId, row: Row, target: Target) -> bool {
        match (row, target) {
            (Row::Tab(tab), Target::Today(at)) => self.drop_into_today(window, tab, at),
            (Row::Tab(tab), Target::Pinned(place)) => self.pin_tab(window, tab, place),
            (Row::Tab(tab), Target::Favorites(at)) => self.favorite_tab(window, tab, at),
            (Row::Pin(PinnedItem::Tab(pin)), Target::Today(at)) => self.unpin(window, pin, at),
            (Row::Pin(item), Target::Pinned(place)) => self.place_pin(window, item, place),
            (Row::Pin(PinnedItem::Tab(pin)), Target::Favorites(at)) => {
                self.favorite_pin(window, pin, at)
            }
            (Row::Pin(PinnedItem::Folder(_)), Target::Today(_) | Target::Favorites(_)) => false,
            (Row::Favorite(favorite), Target::Today(at)) => {
                self.unfavorite_to_today(window, favorite, at)
            }
            (Row::Favorite(favorite), Target::Pinned(place)) => {
                self.unfavorite_to_pinned(window, favorite, place)
            }
            (Row::Favorite(favorite), Target::Favorites(at)) => self.place_favorite(favorite, at),
        }
    }

    pub fn toggle_pin(&mut self, window: WindowId, tab: TabId) -> bool {
        match self.tabs.get(tab).map(|tab_entry| tab_entry.home) {
            Some(Home::Today(_)) => self.pin_tab(window, tab, PinPlace::default()),
            Some(Home::Pinned(pin)) => self.unpin(window, pin, 0),
            Some(Home::Favorite(_)) | None => false,
        }
    }

    pub fn favorite(&mut self, window: WindowId, tab: TabId) -> bool {
        let at = self.favorites_here(window).len();
        match self.tabs.get(tab).map(|tab_entry| tab_entry.home) {
            Some(Home::Today(_)) => self.favorite_tab(window, tab, at),
            Some(Home::Pinned(pin)) => self.favorite_pin(window, pin, at),
            Some(Home::Favorite(_)) | None => false,
        }
    }

    pub fn remove_favorite(&mut self, favorite: FavoriteId) {
        if let Some((_, _, removed)) = self.favorites.remove(favorite) {
            self.store.remove_favorite(favorite);
            self.release(Slot::Favorite(favorite), None, removed.saved);
        }
    }

    fn place_favorite(&mut self, favorite: FavoriteId, at: usize) -> bool {
        match self.favorites.place(favorite, at) {
            Some(profile) => {
                self.save_favorite_order(profile);
                true
            }
            None => false,
        }
    }

    fn add_favorite(&mut self, profile: crate::ProfileId, at: usize, saved: Saved) -> FavoriteId {
        let id = self
            .store
            .add_favorite(profile, saved.url.clone(), saved.title.clone());
        self.favorites.insert(profile, at, Favorite { id, saved });
        self.save_favorite_order(profile);
        id
    }

    fn add_pin(&mut self, space: SpaceId, place: PinPlace, saved: Saved) -> Option<PinnedTabId> {
        let title = Some(saved.title.clone()).filter(|title| !title.is_empty());
        let id = self
            .store
            .pin_tab(kept_place(space, place), saved.url.clone(), title);
        if self
            .pinned
            .insert(space, place, Node::Pin(Pin { id, saved }))
        {
            return Some(id);
        }
        self.store.unpin_tab(id);
        None
    }

    pub fn move_to_space(&mut self, window: WindowId, tab: TabId, space: SpaceId) -> bool {
        let Some(profile) = self.spaces.get(space).map(|space| space.profile) else {
            return false;
        };
        let Some(Home::Today(from)) = self.tabs.get(tab).map(|tab_entry| tab_entry.home) else {
            return false;
        };
        if from == Some(space) {
            return false;
        }
        let neighbour = self.next_after(window, tab);
        let Some(layout) = self.windows.get_mut(window) else {
            return false;
        };
        let was_active = layout.active() == Some(tab);
        if !layout.take(tab) {
            return false;
        }
        layout.insert_today(Some(space), 0, tab);
        self.move_tab_home(
            tab,
            Home::Today(Some(space)),
            ProfileKey::Persistent(profile),
        );
        if was_active {
            self.fall_back(window, neighbour);
        }
        true
    }

    fn drop_into_today(&mut self, window: WindowId, tab: TabId, at: usize) -> bool {
        let Some(home) = self.tabs.get(tab).map(|tab_entry| tab_entry.home) else {
            return false;
        };
        match home {
            Home::Pinned(pin) => self.unpin(window, pin, at),
            Home::Favorite(favorite) => self.unfavorite_to_today(window, favorite, at),
            Home::Today(_) => self.take_into_today(window, tab, at),
        }
    }

    fn take_into_today(&mut self, window: WindowId, tab: TabId, at: usize) -> bool {
        let Some(owner) = self.windows.window_of(tab) else {
            return false;
        };
        let Some(target) = self.windows.get(window) else {
            return false;
        };
        let (private, space) = (target.private, target.space());
        if self
            .windows
            .get(owner)
            .is_none_or(|from| from.private != private)
        {
            return false;
        }
        let at = match target.place_in_today(tab) {
            Some((current_space, from)) if current_space == space && at > from => at - 1,
            _ => at,
        };
        let neighbour = self.next_after(owner, tab);
        let was_active = self
            .windows
            .get(owner)
            .is_some_and(|from| from.active() == Some(tab));
        if let Some(from) = self.windows.get_mut(owner) {
            from.take(tab);
        }
        if owner != window && was_active {
            self.fall_back(owner, neighbour);
        }
        let profile = self.profile_here(window);
        self.move_tab_home(tab, Home::Today(space), profile);
        if let Some(layout) = self.windows.get_mut(window) {
            layout.insert_today(space, at, tab);
            layout.set_active(Some(tab));
        }
        true
    }

    fn pin_tab(&mut self, window: WindowId, tab: TabId, place: PinPlace) -> bool {
        let Some(space) = self.windows.get(window).and_then(|layout| layout.space()) else {
            return false;
        };
        let Some(tab_entry) = self.tabs.get(tab) else {
            return false;
        };
        match tab_entry.home {
            Home::Pinned(pin) => return self.place_pin(window, PinnedItem::Tab(pin), place),
            Home::Favorite(favorite) => return self.unfavorite_to_pinned(window, favorite, place),
            Home::Today(_) => {}
        }
        if self.windows.window_of(tab) != Some(window) {
            return false;
        }
        let saved = tab_entry.saved();
        let Some(pin) = self.add_pin(space, place, saved) else {
            return false;
        };
        let Some(layout) = self.windows.get_mut(window) else {
            return false;
        };
        let was_active = layout.active() == Some(tab);
        layout.take(tab);
        layout.open_slot(Slot::Pinned(pin), tab);
        if was_active {
            layout.set_active(Some(tab));
        }
        let profile = self.profile_here(window);
        self.move_tab_home(tab, Home::Pinned(pin), profile);
        true
    }

    fn place_pin(&mut self, window: WindowId, item: PinnedItem, place: PinPlace) -> bool {
        let Some(space) = self.windows.get(window).and_then(|layout| layout.space()) else {
            return false;
        };
        if !self.pinned.place(item, space, place) {
            return false;
        }
        self.save_pinned_place(item);
        let profile = self.spaces.get(space).map(|space| space.profile);
        let mut pins = Vec::new();
        if let Some((_, node)) = self.pinned.find(item) {
            node.pins(&mut pins);
        }
        for pin in pins {
            for tab in self.instances(Slot::Pinned(pin)) {
                if let Some(profile) = profile {
                    self.move_tab_home(tab, Home::Pinned(pin), ProfileKey::Persistent(profile));
                }
            }
        }
        true
    }

    fn unpin(&mut self, window: WindowId, pin: PinnedTabId, at: usize) -> bool {
        let Some((_, _, Node::Pin(removed))) = self.pinned.remove(PinnedItem::Tab(pin)) else {
            return false;
        };
        self.store.unpin_tab(pin);
        self.release(Slot::Pinned(pin), Some((window, at)), removed.saved);
        true
    }

    fn favorite_tab(&mut self, window: WindowId, tab: TabId, at: usize) -> bool {
        let ProfileKey::Persistent(profile) = self.profile_here(window) else {
            return false;
        };
        let Some(tab_entry) = self.tabs.get(tab) else {
            return false;
        };
        match tab_entry.home {
            Home::Pinned(pin) => return self.favorite_pin(window, pin, at),
            Home::Favorite(favorite) => return self.place_favorite(favorite, at),
            Home::Today(_) => {}
        }
        if self.windows.window_of(tab) != Some(window) {
            return false;
        }
        let saved = tab_entry.saved();
        let id = self.add_favorite(profile, at, saved);
        let Some(layout) = self.windows.get_mut(window) else {
            return false;
        };
        let was_active = layout.active() == Some(tab);
        layout.take(tab);
        layout.open_slot(Slot::Favorite(id), tab);
        if was_active {
            layout.set_active(Some(tab));
        }
        self.move_tab_home(tab, Home::Favorite(id), ProfileKey::Persistent(profile));
        true
    }

    fn favorite_pin(&mut self, window: WindowId, pin: PinnedTabId, at: usize) -> bool {
        let ProfileKey::Persistent(profile) = self.profile_here(window) else {
            return false;
        };
        let Some((_, _, Node::Pin(removed))) = self.pinned.remove(PinnedItem::Tab(pin)) else {
            return false;
        };
        self.store.unpin_tab(pin);
        let id = self.add_favorite(profile, at, removed.saved);
        for layout in self.windows.iter_mut() {
            layout.move_slot(Slot::Pinned(pin), Slot::Favorite(id));
        }
        for tab in self.instances(Slot::Favorite(id)) {
            self.move_tab_home(tab, Home::Favorite(id), ProfileKey::Persistent(profile));
        }
        true
    }

    fn unfavorite_to_today(&mut self, window: WindowId, favorite: FavoriteId, at: usize) -> bool {
        let Some((_, _, removed)) = self.favorites.remove(favorite) else {
            return false;
        };
        self.store.remove_favorite(favorite);
        self.release(Slot::Favorite(favorite), Some((window, at)), removed.saved);
        true
    }

    fn unfavorite_to_pinned(
        &mut self,
        window: WindowId,
        favorite: FavoriteId,
        place: PinPlace,
    ) -> bool {
        let Some(space) = self.windows.get(window).and_then(|layout| layout.space()) else {
            return false;
        };
        let Some(profile) = self.spaces.get(space).map(|space| space.profile) else {
            return false;
        };
        let Some((_, _, removed)) = self.favorites.remove(favorite) else {
            return false;
        };
        self.store.remove_favorite(favorite);
        let Some(pin) = self.add_pin(space, place, removed.saved) else {
            return false;
        };
        for layout in self.windows.iter_mut() {
            layout.move_slot(Slot::Favorite(favorite), Slot::Pinned(pin));
        }
        for tab in self.instances(Slot::Pinned(pin)) {
            self.move_tab_home(tab, Home::Pinned(pin), ProfileKey::Persistent(profile));
        }
        true
    }

    pub fn release(&mut self, slot: Slot, into: Option<(WindowId, usize)>, saved: Saved) {
        let owners: Vec<(WindowId, TabId)> = self
            .windows
            .iter()
            .filter_map(|layout| layout.tab_in_slot(slot).map(|tab| (layout.id, tab)))
            .collect();
        let mut placed = false;
        for (owner, tab) in owners {
            let at = match into {
                Some((window, at)) if window == owner => {
                    placed = true;
                    at
                }
                _ => 0,
            };
            let profile = self.profile_here(owner);
            let Some(layout) = self.windows.get_mut(owner) else {
                continue;
            };
            let was_active = layout.active() == Some(tab);
            layout.take(tab);
            let space = layout.space();
            layout.insert_today(space, at, tab);
            if was_active {
                layout.set_active(Some(tab));
            }
            self.move_tab_home(tab, Home::Today(space), profile);
        }
        let Some((window, at)) = into.filter(|_| !placed) else {
            return;
        };
        let profile = self.profile_here(window);
        let Some(space) = self.windows.get(window).map(Layout::space) else {
            return;
        };
        let id = self.insert_tab(profile, Home::Today(space), saved);
        if let Some(layout) = self.windows.get_mut(window) {
            layout.insert_today(space, at, id);
        }
    }

    fn instances(&self, slot: Slot) -> Vec<TabId> {
        self.windows
            .iter()
            .filter_map(|layout| layout.tab_in_slot(slot))
            .collect()
    }

    pub fn move_tab_home(&mut self, tab: TabId, home: Home, profile: ProfileKey) {
        let Some(tab_entry) = self.tabs.get_mut(tab) else {
            return;
        };
        tab_entry.home = home;
        if tab_entry.profile != profile {
            tab_entry.profile = profile;
            tab_entry.set_page(Page::Unloaded);
        }
    }

    fn next_after(&self, window: WindowId, tab: TabId) -> Option<TabId> {
        let layout = self.windows.get(window)?;
        let today = layout.today_here();
        let at = today.iter().position(|candidate| *candidate == tab)?;
        today
            .get(at + 1)
            .or_else(|| at.checked_sub(1).and_then(|before| today.get(before)))
            .copied()
    }
}
