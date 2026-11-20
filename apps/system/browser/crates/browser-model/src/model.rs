use std::collections::HashMap;

use browser_store::{SpaceFields, Store};

use crate::ids::WindowIds;
use crate::names::Names;
use crate::page::LivePage;
use crate::split::Splits;
use crate::{
    Favorite, Favorites, Home, Layout, Loaded, Node, PinPlace, Pinned, PinnedItem, Profile,
    ProfileId, ProfileKey, Profiles, Saved, Slot, Space, SpaceIcon, SpaceId, Spaces, Symbol, Tab,
    TabId, Tabs, WindowId, Windows, Zooms, same_zoom,
};

#[derive(Debug)]
pub struct Model<L> {
    pub profiles: Profiles,
    pub spaces: Spaces,
    pub favorites: Favorites,
    pub pinned: Pinned,
    pub tabs: Tabs<L>,
    pub windows: Windows,
    pub zooms: Zooms,
    pub splits: Splits,
    pub(crate) store: Store,
    pub(crate) window_ids: WindowIds,
    pub(crate) names: Names,
}

impl<L: LivePage> Model<L> {
    pub fn load(store: Store, loaded: &Loaded, names: Names) -> Model<L> {
        let mut profiles: Vec<Profile> = loaded
            .profiles
            .iter()
            .map(|profile| Profile {
                id: profile.id,
                name: profile.name.clone(),
            })
            .collect();
        if profiles.is_empty() {
            let name = (names.profile)();
            let id = store.create_profile(name.clone());
            profiles.push(Profile { id, name });
        }
        let mut spaces: Vec<Space> = loaded
            .spaces
            .iter()
            .filter(|space| profiles.iter().any(|profile| profile.id == space.profile))
            .map(|space| {
                let fields = SpaceFields {
                    name: space.name.clone(),
                    icon: space.icon.clone(),
                    profile: space.profile,
                };
                Space::from_store(space.id, &fields)
            })
            .collect();
        if spaces.is_empty() {
            let fields = SpaceFields {
                name: (names.space)(),
                icon: SpaceIcon::Symbol(Symbol::Star).to_store(),
                profile: profiles[0].id,
            };
            let id = store.create_space(fields.clone());
            spaces.push(Space::from_store(id, &fields));
        }
        let mut favorites = Favorites::default();
        for favorite in &loaded.favorites {
            let at = favorites.for_profile(favorite.profile).len();
            let saved = Saved {
                url: favorite.url.clone(),
                title: favorite.title.clone(),
                favicon: loaded.favicon(&favorite.url),
            };
            favorites.insert(
                favorite.profile,
                at,
                Favorite {
                    id: favorite.id,
                    saved,
                },
            );
        }
        let pinned = loaded
            .pinned
            .iter()
            .map(|(space, nodes)| {
                let mut nodes: Vec<Node> = nodes.iter().cloned().map(Node::from_store).collect();
                for node in &mut nodes {
                    node.saved_mut(&mut |saved: &mut Saved| {
                        saved.favicon = loaded.favicon(&saved.url);
                    });
                }
                (*space, nodes)
            })
            .collect::<HashMap<_, _>>();
        Model {
            profiles: Profiles::new(profiles),
            spaces: Spaces::new(spaces),
            favorites,
            pinned: Pinned::new(pinned),
            tabs: Tabs::default(),
            windows: Windows::default(),
            zooms: Zooms::new(loaded.zooms.clone()),
            splits: Splits::default(),
            store,
            window_ids: WindowIds::default(),
            names,
        }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn window(&self, id: WindowId) -> Option<&Layout> {
        self.windows.get(id)
    }

    pub fn tab(&self, id: TabId) -> Option<&Tab<L>> {
        self.tabs.get(id)
    }

    pub fn tab_mut(&mut self, id: TabId) -> Option<&mut Tab<L>> {
        self.tabs.get_mut(id)
    }

    pub fn active_mut(&mut self, window: WindowId) -> Option<&mut Tab<L>> {
        let id = self.window(window)?.active()?;
        self.tabs.get_mut(id)
    }

    pub fn active(&self, window: WindowId) -> Option<&Tab<L>> {
        self.tab(self.window(window)?.active()?)
    }

    pub fn space_of(&self, window: WindowId) -> Option<&Space> {
        self.spaces.get(self.window(window)?.space()?)
    }

    pub fn profile_here(&self, window: WindowId) -> ProfileKey {
        match self.space_of(window) {
            Some(space) => ProfileKey::Persistent(space.profile),
            None => ProfileKey::Private,
        }
    }

    pub fn favorites_here(&self, window: WindowId) -> &[Favorite] {
        match self.profile_here(window) {
            ProfileKey::Persistent(profile) => self.favorites.for_profile(profile),
            ProfileKey::Private => &[],
        }
    }

    pub fn strip(&self, window: WindowId) -> Vec<TabId> {
        let Some(layout) = self.window(window) else {
            return Vec::new();
        };
        let favorites = self
            .favorites_here(window)
            .iter()
            .filter_map(|favorite| layout.tab_in_slot(Slot::Favorite(favorite.id)));
        let pinned = layout
            .space()
            .map(|space| self.pinned.pins(space))
            .unwrap_or_default()
            .into_iter()
            .filter_map(|pin| layout.tab_in_slot(Slot::Pinned(pin)));
        favorites
            .chain(pinned)
            .chain(layout.today_here().iter().copied())
            .collect()
    }

    pub fn add_window(&mut self, private: bool) -> WindowId {
        let id = self.window_ids.next();
        let space = match private {
            true => None,
            false => Some(
                self.windows
                    .main()
                    .and_then(|main| self.windows.get(main)?.space())
                    .unwrap_or_else(|| self.spaces.first()),
            ),
        };
        self.windows.add(Layout::new(id, private, space));
        id
    }

    pub fn remove_window(&mut self, id: WindowId) {
        let Some(layout) = self.windows.remove(id) else {
            return;
        };
        for tab in layout.tabs() {
            self.tabs.remove(tab);
        }
    }

    pub fn focused(&mut self, window: WindowId) {
        self.windows.focused(window);
    }

    pub fn writes(&self, window: WindowId) -> bool {
        self.window(window)
            .is_some_and(|layout| !layout.private && !layout.little)
    }

    pub fn zoom_for(&self, tab: TabId, default: f64) -> f64 {
        let Some(tab_entry) = self.tab(tab) else {
            return default;
        };
        let (Some(profile), Some(host)) = (
            tab_entry.persistent_profile(),
            browser_pages::web_host(&tab_entry.meta.url),
        ) else {
            return default;
        };
        self.zooms.get(profile, &host).unwrap_or(default)
    }

    pub fn remember_home(&mut self, tab: TabId) {
        let Some(tab_entry) = self.tab(tab) else {
            return;
        };
        let (home, saved) = (tab_entry.home, tab_entry.meta.saved());
        match home {
            Home::Pinned(pin) => self.pinned.remember(pin, &saved),
            Home::Favorite(favorite) => self.favorites.remember(favorite, &saved),
            Home::Today(_) => {}
        }
    }

    pub fn remember_zoom(&mut self, tab: TabId, zoom: f64, default: f64) {
        let Some(tab_entry) = self.tab(tab) else {
            return;
        };
        let (Some(profile), Some(host)) = (
            tab_entry.persistent_profile(),
            browser_pages::web_host(&tab_entry.meta.url),
        ) else {
            return;
        };
        if same_zoom(zoom, default) {
            self.zooms.set(profile, &host, None);
            self.store.clear_zoom(profile, &host);
            return;
        }
        self.zooms.set(profile, &host, Some(zoom));
        if let Err(error) = self.store.set_zoom(profile, &host, zoom) {
            tracing::warn!(%error, "failed to store zoom level");
        }
    }

    pub fn insert_linked_tab(&mut self, profile: ProfileKey, home: Home, url: String) -> TabId {
        let id = self.insert_tab(profile, home, Saved::new(url, ""));
        if let Some(tab_entry) = self.tab_mut(id) {
            tab_entry
                .visit_tracker
                .set_cause(browser_pages::Cause::Link);
        }
        id
    }

    pub(crate) fn insert_tab(&mut self, profile: ProfileKey, home: Home, saved: Saved) -> TabId {
        let id = self.store.new_tab_id();
        self.tabs.insert(Tab::new(id, profile, home, saved));
        id
    }

    pub(crate) fn save_space(&self, space: SpaceId) {
        if let Some(space_entry) = self.spaces.get(space) {
            self.store.update_space(space, space_entry.to_fields());
        }
    }

    pub(crate) fn save_favorite_order(&self, profile: ProfileId) {
        self.store
            .reorder_favorites(profile, &self.favorites.order(profile));
    }

    pub(crate) fn save_pinned_place(&self, item: PinnedItem) {
        if let Some((space, place)) = self.pinned.place_of(item) {
            self.store.move_pinned(item, kept_place(space, place));
        }
    }
}

pub(crate) fn kept_place(space: SpaceId, place: PinPlace) -> browser_store::PinnedPlace {
    let parent = match place.parent {
        Some(folder) => browser_store::PinnedPlace::folder(folder),
        None => browser_store::PinnedPlace::root(space),
    };
    parent.at(place.index)
}
