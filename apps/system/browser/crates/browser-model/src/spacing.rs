use browser_store::{FolderRemoval, SpaceFields};
use rikka_types::Choice;

use crate::model::kept_place;
use crate::page::LivePage;
use crate::{
    Folder, FolderId, Model, Node, PinPlace, PinnedItem, Profile, ProfileId, ProfileKey, Slot,
    Space, SpaceIcon, SpaceId, Symbol, WindowId,
};

impl<L: LivePage> Model<L> {
    pub fn switch_space(&mut self, window: WindowId, space: SpaceId) {
        if self.spaces.get(space).is_none() {
            return;
        }
        let Some(layout) = self.windows.get_mut(window) else {
            return;
        };
        if layout.private || layout.space() == Some(space) {
            return;
        }
        layout.set_space(Some(space));
        let next = layout
            .recent(Some(space))
            .filter(|tab| layout.holds(*tab))
            .or_else(|| layout.today(Some(space)).first().copied());
        layout.set_active(next);
    }

    pub fn step_space(&mut self, window: WindowId, by: isize) {
        let next = self
            .windows
            .get(window)
            .and_then(|layout| layout.space())
            .and_then(|space| self.spaces.step(space, by));
        if let Some(next) = next {
            self.switch_space(window, next);
        }
    }

    pub fn add_space(&mut self, window: WindowId) -> Option<SpaceId> {
        let profile = self.space_of(window)?.profile;
        let name = (self.names.numbered_space)(self.spaces.list().len() + 1);
        let id = self.create_space(profile, name);
        self.switch_space(window, id);
        Some(id)
    }

    pub(crate) fn create_space(&mut self, profile: ProfileId, name: String) -> SpaceId {
        let number = self.spaces.list().len() + 1;
        let fields = SpaceFields {
            name,
            icon: SpaceIcon::Symbol(Symbol::ALL[number % Symbol::ALL.len()]).to_store(),
            profile,
        };
        let id = self.store.create_space(fields.clone());
        self.spaces.add(Space::from_store(id, &fields));
        id
    }

    pub fn rename_space(&mut self, space: SpaceId, name: &str) {
        let name = name.trim();
        if !name.is_empty() {
            self.spaces
                .edit(space, |space_entry| space_entry.name = name.to_owned());
            self.save_space(space);
        }
    }

    pub fn set_space_icon(&mut self, space: SpaceId, icon: SpaceIcon) {
        self.spaces
            .edit(space, |space_entry| space_entry.icon = icon);
        self.save_space(space);
    }

    pub fn set_space_profile(&mut self, space: SpaceId, profile: ProfileId) {
        if self.profiles.get(profile).is_none() {
            return;
        }
        self.spaces
            .edit(space, |tab_entry| tab_entry.profile = profile);
        self.save_space(space);
        let pins = self.pinned.pins(space);
        let today_tabs: Vec<_> = self
            .windows
            .iter()
            .flat_map(|layout| {
                let mut tabs = layout.today(Some(space)).to_vec();
                tabs.extend(
                    pins.iter()
                        .filter_map(|pin| layout.tab_in_slot(Slot::Pinned(*pin))),
                );
                tabs
            })
            .collect();
        for tab in today_tabs {
            if let Some(home) = self.tabs.get(tab).map(|tab_entry| tab_entry.home) {
                self.move_tab_home(tab, home, ProfileKey::Persistent(profile));
            }
        }
    }

    pub fn delete_space(&mut self, space: SpaceId) -> Vec<L::View> {
        let Some(at) = self.spaces.position(space) else {
            return Vec::new();
        };
        if self.spaces.list().len() <= 1 {
            return Vec::new();
        }
        let mut closing = Vec::new();
        let windows: Vec<WindowId> = self.windows.iter().map(|layout| layout.id).collect();
        let pins = self.pinned.pins(space);
        for window in windows {
            let Some(layout) = self.windows.get(window) else {
                continue;
            };
            let mut tabs = layout.today(Some(space)).to_vec();
            tabs.extend(
                pins.iter()
                    .filter_map(|pin| layout.tab_in_slot(Slot::Pinned(*pin))),
            );
            for tab in tabs {
                closing.extend(self.close_without_archive(window, tab));
            }
        }
        self.pinned.remove_space(space);
        self.spaces.remove(space);
        self.store.remove_space(space);
        let successor = self.spaces.list()[at.min(self.spaces.list().len() - 1)].id;
        let showing: Vec<WindowId> = self
            .windows
            .iter()
            .filter(|layout| layout.space() == Some(space))
            .map(|layout| layout.id)
            .collect();
        for window in showing {
            self.switch_space(window, successor);
        }
        closing
    }

    pub fn add_folder(&mut self, window: WindowId, place: PinPlace) -> Option<FolderId> {
        let space = self.windows.get(window)?.space()?;
        let name = (self.names.folder)();
        let id = self.insert_folder(space, place, name)?;
        if let Some(parent) = place.parent {
            self.open_folder(parent);
        }
        Some(id)
    }

    pub(crate) fn insert_folder(
        &mut self,
        space: SpaceId,
        place: PinPlace,
        name: String,
    ) -> Option<FolderId> {
        let id = self
            .store
            .create_pinned_folder(kept_place(space, place), name.clone());
        let folder = Node::Folder(Folder {
            id,
            name,
            open: true,
            children: Vec::new(),
        });
        if !self.pinned.insert(space, place, folder) {
            self.store
                .remove_pinned_folder(id, FolderRemoval::WithContents);
            return None;
        }
        Some(id)
    }

    fn open_folder(&mut self, folder: FolderId) {
        if self
            .pinned
            .edit_folder(folder, |folder_entry| folder_entry.open = true)
        {
            self.store.set_pinned_folder_expanded(folder, true);
        }
    }

    pub fn rename_folder(&mut self, folder: FolderId, name: &str) {
        let name = name.trim();
        if !name.is_empty()
            && self
                .pinned
                .edit_folder(folder, |folder_entry| folder_entry.name = name.to_owned())
        {
            self.store.rename_pinned_folder(folder, name);
        }
    }

    pub fn toggle_folder(&mut self, folder: FolderId) {
        let mut open = false;
        let changed = self.pinned.edit_folder(folder, |folder_entry| {
            folder_entry.open = !folder_entry.open;
            open = folder_entry.open;
        });
        if changed {
            self.store.set_pinned_folder_expanded(folder, open);
        }
    }

    pub fn delete_folder(&mut self, folder: FolderId) {
        let Some((_, _, removed)) = self.pinned.remove(PinnedItem::Folder(folder)) else {
            return;
        };
        self.store
            .remove_pinned_folder(folder, FolderRemoval::WithContents);
        let mut pins = Vec::new();
        removed.pins(&mut pins);
        for pin in pins {
            self.release(Slot::Pinned(pin), None, crate::Saved::default());
        }
    }

    pub fn add_profile(&mut self) -> ProfileId {
        let number = self.profiles.list().len() + 1;
        let name = (self.names.numbered_profile)(number);
        let id = self.store.create_profile(name.clone());
        self.profiles.add(Profile { id, name });
        id
    }

    pub fn rename_profile(&mut self, profile: ProfileId, name: &str) {
        let name = name.trim();
        if !name.is_empty() {
            self.profiles.rename(profile, name.to_owned());
            self.store.rename_profile(profile, name);
        }
    }

    pub fn removable_profile(&self, profile: ProfileId) -> bool {
        self.profiles.list().len() > 1
            && !self
                .spaces
                .list()
                .iter()
                .any(|space| space.profile == profile)
    }

    pub fn forget_profile(&mut self, profile: ProfileId) {
        self.profiles.remove(profile);
        while let Some(favorite) = self
            .favorites
            .for_profile(profile)
            .first()
            .map(|first| first.id)
        {
            self.favorites.remove(favorite);
        }
    }
}
