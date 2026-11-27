use browser_bookmarks::{
    LocalSide, Mirror, MirrorFavorite, MirrorNode, MirrorSpace, Pending, Plan,
};
use browser_store::{PinnedParent, PinnedPlace};

use crate::page::LivePage;
use crate::{
    FavoriteId, FolderId, Model, Node, PinPlace, PinnedItem, PinnedTabId, ProfileId, Saved, SpaceId,
};

impl<L: LivePage> Model<L> {
    pub fn bookmark_mirror(&self, profile: ProfileId) -> Mirror {
        Mirror {
            favorites: self
                .favorites
                .for_profile(profile)
                .iter()
                .map(|favorite| MirrorFavorite {
                    id: favorite.id,
                    url: favorite.saved.url.clone(),
                    title: favorite.saved.title.clone(),
                })
                .collect(),
            spaces: self
                .spaces
                .list()
                .iter()
                .filter(|space| space.profile == profile)
                .map(|space| MirrorSpace {
                    id: space.id,
                    name: space.name.clone(),
                    pinned: mirror_nodes(self.pinned.tree(space.id)),
                })
                .collect(),
        }
    }

    pub fn apply_bookmarks(&mut self, profile: ProfileId, plan: Plan) -> Pending {
        plan.apply_local(&mut ProfileBookmarks {
            model: self,
            profile,
        })
    }

    fn pin_place(&self, place: PinnedPlace) -> Option<(SpaceId, PinPlace)> {
        let (space, parent) = match place.parent {
            PinnedParent::Root(space) => (space, None),
            PinnedParent::Folder(folder) => {
                let (space, _) = self.pinned.find(PinnedItem::Folder(folder))?;
                (space, Some(folder))
            }
        };
        let index = place.index.unwrap_or(usize::MAX);
        Some((space, PinPlace { parent, index }))
    }
}

fn mirror_nodes(nodes: &[Node]) -> Vec<MirrorNode> {
    nodes
        .iter()
        .map(|node| match node {
            Node::Folder(folder) => MirrorNode::Folder {
                id: folder.id,
                name: folder.name.clone(),
                children: mirror_nodes(&folder.children),
            },
            Node::Pin(pin) => MirrorNode::Pin {
                id: pin.id,
                url: pin.saved.url.clone(),
                title: pin.saved.title.clone(),
            },
        })
        .collect()
}

struct ProfileBookmarks<'a, L> {
    model: &'a mut Model<L>,
    profile: ProfileId,
}

impl<L: LivePage> LocalSide for ProfileBookmarks<'_, L> {
    fn add_favorite(&mut self, url: &str, title: &str) -> Option<FavoriteId> {
        let at = self.model.favorites.for_profile(self.profile).len();
        Some(
            self.model
                .add_favorite(self.profile, at, Saved::new(url, title)),
        )
    }

    fn edit_favorite(&mut self, id: FavoriteId, url: &str, title: &str) {
        self.model.favorites.retitle(id, url, title);
        self.model.store.update_favorite(id, url, title);
    }

    fn remove_favorite(&mut self, id: FavoriteId) {
        self.model.remove_favorite(id);
    }

    fn order_favorites(&mut self, order: &[FavoriteId]) {
        self.model.favorites.arrange(self.profile, order);
        self.model.save_favorite_order(self.profile);
    }

    fn add_space(&mut self, name: &str) -> Option<SpaceId> {
        Some(self.model.create_space(self.profile, name.to_owned()))
    }

    fn rename_space(&mut self, id: SpaceId, name: &str) {
        self.model.rename_space(id, name);
    }

    fn add_folder(&mut self, place: PinnedPlace, name: &str) -> Option<FolderId> {
        let (space, place) = self.model.pin_place(place)?;
        self.model.insert_folder(space, place, name.to_owned())
    }

    fn rename_folder(&mut self, id: FolderId, name: &str) {
        self.model.rename_folder(id, name);
    }

    fn add_pin(&mut self, place: PinnedPlace, url: &str, title: &str) -> Option<PinnedTabId> {
        let (space, place) = self.model.pin_place(place)?;
        self.model.add_pin(space, place, Saved::new(url, title))
    }

    fn edit_pin(&mut self, id: PinnedTabId, url: &str, title: &str) {
        self.model.pinned.retitle(id, url, title);
        let title = Some(title.to_owned()).filter(|title| !title.is_empty());
        self.model.store.update_pinned_tab(id, url, title);
    }

    fn move_pinned(&mut self, item: PinnedItem, place: PinnedPlace) {
        if let Some((space, place)) = self.model.pin_place(place) {
            self.model.move_pinned_to(item, space, place);
        }
    }

    fn remove_pinned(&mut self, item: PinnedItem) {
        match item {
            PinnedItem::Folder(folder) => self.model.delete_folder(folder),
            PinnedItem::Tab(pin) => {
                self.model.release_pin(pin, None);
            }
        }
    }
}
