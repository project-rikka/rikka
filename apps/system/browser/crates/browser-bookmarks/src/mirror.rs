use browser_store::{FavoriteId, FolderId, PinnedTabId, SpaceId};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mirror {
    pub favorites: Vec<MirrorFavorite>,
    pub spaces: Vec<MirrorSpace>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorFavorite {
    pub id: FavoriteId,
    pub url: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorSpace {
    pub id: SpaceId,
    pub name: String,
    pub pinned: Vec<MirrorNode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MirrorNode {
    Folder {
        id: FolderId,
        name: String,
        children: Vec<MirrorNode>,
    },
    Pin {
        id: PinnedTabId,
        url: String,
        title: String,
    },
}
