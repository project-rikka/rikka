use rikka_app::prelude::*;

use crate::shell::Shell;
use crate::sidebar::Renaming;
use browser_model::{
    FavoriteId, FolderId, PinPlace, PinnedItem, PinnedTabId, ProfileId, Row, SpaceIcon, SpaceId,
    Target,
};

impl Shell {
    pub(crate) fn rename(&mut self, target: Renaming, current: &str) {
        self.sidebar.rename(target, current);
    }

    pub(crate) fn finish_renaming(&mut self) {
        let Some((target, name)) = self.sidebar.finish_renaming() else {
            return;
        };
        let mut model = self.browser.edit();
        match target {
            Renaming::Space(space) => model.rename_space(space, &name),
            Renaming::Folder(folder) => model.rename_folder(folder, &name),
            Renaming::Profile(profile) => model.rename_profile(profile, &name),
        }
    }

    pub(crate) fn abandon_renaming(&mut self) {
        self.sidebar.abandon_renaming();
    }

    pub(crate) fn new_folder(&mut self, within: Option<FolderId>) {
        let place = PinPlace {
            parent: within,
            index: 0,
        };
        let added = self.browser.edit().add_folder(self.id, place);
        if let Some(folder) = added {
            let name = tr!("browser.folder.new").to_string();
            self.sidebar.rename(Renaming::Folder(folder), &name);
        }
    }

    pub(crate) fn toggle_folder(&mut self, folder: FolderId) {
        self.browser.edit().toggle_folder(folder);
    }

    pub(crate) fn delete_folder(&mut self, folder: FolderId) {
        self.browser.edit().delete_folder(folder);
        self.changed();
    }

    pub(crate) fn unpin(&mut self, pin: PinnedTabId) {
        self.dropped(Row::Pin(PinnedItem::Tab(pin)), Target::Today(0));
    }

    pub(crate) fn unfavorite(&mut self, favorite: FavoriteId) {
        self.browser.edit().remove_favorite(favorite);
        self.changed();
    }

    pub(crate) fn switch_space(&mut self, space: SpaceId) {
        self.browser.edit().switch_space(self.id, space);
        self.changed();
    }

    pub(crate) fn new_space(&mut self) {
        let added = self.browser.edit().add_space(self.id);
        if let Some(space) = added {
            let name = self
                .model()
                .spaces
                .get(space)
                .map(|space_entry| space_entry.name.clone())
                .unwrap_or_default();
            self.sidebar.rename(Renaming::Space(space), &name);
        }
        self.changed();
    }

    pub(crate) fn set_space_icon(&mut self, space: SpaceId, icon: SpaceIcon) {
        self.browser.edit().set_space_icon(space, icon);
    }

    pub(crate) fn set_space_profile(&mut self, space: SpaceId, profile: ProfileId) {
        self.browser.edit().set_space_profile(space, profile);
        self.changed();
    }

    pub(crate) fn new_profile_for(&mut self, space: SpaceId) {
        let profile = self.browser.edit().add_profile();
        self.set_space_profile(space, profile);
    }

    pub(crate) fn new_profile(&mut self) {
        let profile = self.browser.edit().add_profile();
        let name = self
            .model()
            .profiles
            .get(profile)
            .map(|profile_entry| profile_entry.name.clone())
            .unwrap_or_default();
        self.sidebar.rename(Renaming::Profile(profile), &name);
    }

    pub(crate) fn delete_space(&mut self, space: SpaceId) {
        let closing = self.browser.edit().delete_space(space);
        self.close_views(closing);
        self.changed();
    }
}
