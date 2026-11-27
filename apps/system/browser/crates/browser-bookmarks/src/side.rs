use browser_store::{FavoriteId, FolderId, PinnedItem, PinnedPlace, PinnedTabId, SpaceId};

pub trait LocalSide {
    fn add_favorite(&mut self, url: &str, title: &str) -> Option<FavoriteId>;
    fn edit_favorite(&mut self, id: FavoriteId, url: &str, title: &str);
    fn remove_favorite(&mut self, id: FavoriteId);
    fn order_favorites(&mut self, order: &[FavoriteId]);
    fn add_space(&mut self, name: &str) -> Option<SpaceId>;
    fn rename_space(&mut self, id: SpaceId, name: &str);
    fn add_folder(&mut self, place: PinnedPlace, name: &str) -> Option<FolderId>;
    fn rename_folder(&mut self, id: FolderId, name: &str);
    fn add_pin(&mut self, place: PinnedPlace, url: &str, title: &str) -> Option<PinnedTabId>;
    fn edit_pin(&mut self, id: PinnedTabId, url: &str, title: &str);
    fn move_pinned(&mut self, item: PinnedItem, place: PinnedPlace);
    fn remove_pinned(&mut self, item: PinnedItem);
}
