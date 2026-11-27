use std::future::{Future, ready};
use std::pin::pin;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

use browser_bookmarks::{LocalSide, Mirror, MirrorFavorite, MirrorNode, MirrorSpace, Plan, Remote};
use browser_store::{
    BookmarkSync, FavoriteId, FolderId, PinnedItem, PinnedParent, PinnedPlace, PinnedTabId, SpaceId,
};
use rikka_web::{BookmarkId, BookmarkNode, FolderKind};

pub(crate) fn node(
    id: &str,
    title: &str,
    url: Option<&str>,
    children: Vec<BookmarkNode>,
) -> BookmarkNode {
    BookmarkNode {
        id: BookmarkId::from_raw(id),
        parent_id: None,
        index: 0,
        title: title.to_owned(),
        url: url.map(str::to_owned),
        folder_kind: None,
        date_added: None,
        children,
    }
}

fn root_folder(id: &str, title: &str, kind: FolderKind) -> BookmarkNode {
    BookmarkNode {
        folder_kind: Some(kind),
        ..node(id, title, None, Vec::new())
    }
}

struct Tree {
    root: BookmarkNode,
    next: u64,
    changes: usize,
}

pub(crate) struct Chrome {
    tree: Mutex<Tree>,
}

impl Chrome {
    pub(crate) fn new() -> Chrome {
        let mut root = node(
            "0",
            "",
            None,
            vec![
                root_folder("1", "ブックマーク バー", FolderKind::BookmarksBar),
                root_folder("2", "その他のブックマーク", FolderKind::Other),
                root_folder("3", "モバイル ブックマーク", FolderKind::Mobile),
            ],
        );
        renumber(&mut root);
        Chrome {
            tree: Mutex::new(Tree {
                root,
                next: 100,
                changes: 0,
            }),
        }
    }

    pub(crate) fn tree(&self) -> Vec<BookmarkNode> {
        vec![self.tree.lock().unwrap().root.clone()]
    }

    pub(crate) fn changes(&self) -> usize {
        self.tree.lock().unwrap().changes
    }

    pub(crate) fn id(&self, title: &str) -> BookmarkId {
        fn search(node: &BookmarkNode, title: &str) -> Option<BookmarkId> {
            if node.title == title {
                return Some(node.id.clone());
            }
            node.children.iter().find_map(|child| search(child, title))
        }
        search(&self.tree.lock().unwrap().root, title).expect("titled bookmark")
    }

    pub(crate) fn add(&self, parent: &str, title: &str, url: Option<&str>) -> BookmarkId {
        let parent = self.id(parent);
        self.apply(|tree| create(tree, &parent, u32::MAX, title, url))
            .expect("parent folder")
    }

    pub(crate) fn rename_titled(&self, title: &str, to: &str) {
        let id = self.id(title);
        self.apply(|tree| edit(tree, &id, |node| node.title = to.to_owned()))
            .expect("bookmark");
    }

    pub(crate) fn move_titled(&self, title: &str, parent: &str, index: u32) {
        let (id, parent) = (self.id(title), self.id(parent));
        self.apply(|tree| relocate(tree, &id, &parent, index))
            .expect("bookmark");
    }

    pub(crate) fn remove_titled(&self, title: &str) {
        let id = self.id(title);
        self.apply(|tree| detach(&mut tree.root, &id).map(|_| ()))
            .expect("bookmark");
    }

    pub(crate) fn outline(&self) -> String {
        let tree = self.tree.lock().unwrap();
        let folder = |parent: &BookmarkNode, title: &str| {
            parent
                .children
                .iter()
                .find(|node| node.url.is_none() && node.title == title)
                .cloned()
        };
        let Some(rikka) = folder(&tree.root.children[1], "Rikka") else {
            return String::new();
        };
        let favorites = folder(&rikka, "Favorites").expect("favorites folder");
        let spaces = folder(&rikka, "Spaces").expect("spaces folder");
        let mut parts = vec![remote_nodes(&favorites.children)];
        parts.extend(
            spaces
                .children
                .iter()
                .map(|space| format!("{}: {}", space.title, remote_nodes(&space.children))),
        );
        parts.join(" | ")
    }

    fn apply<T>(&self, change: impl FnOnce(&mut Tree) -> Option<T>) -> Option<T> {
        let mut tree = self.tree.lock().unwrap();
        let changed = change(&mut tree);
        if changed.is_some() {
            tree.changes += 1;
            renumber(&mut tree.root);
        }
        changed
    }
}

fn remote_nodes(nodes: &[BookmarkNode]) -> String {
    nodes
        .iter()
        .map(|node| match node.url {
            Some(_) => node.title.clone(),
            None => format!("{}({})", node.title, remote_nodes(&node.children)),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn renumber(node: &mut BookmarkNode) {
    for (index, child) in (0_u32..).zip(&mut node.children) {
        child.index = index;
        child.parent_id = Some(node.id.clone());
        renumber(child);
    }
}

fn find_mut<'a>(node: &'a mut BookmarkNode, id: &BookmarkId) -> Option<&'a mut BookmarkNode> {
    if node.id == *id {
        return Some(node);
    }
    node.children
        .iter_mut()
        .find_map(|child| find_mut(child, id))
}

fn detach(node: &mut BookmarkNode, id: &BookmarkId) -> Option<(BookmarkNode, BookmarkId, usize)> {
    if let Some(at) = node.children.iter().position(|child| child.id == *id) {
        return Some((node.children.remove(at), node.id.clone(), at));
    }
    node.children.iter_mut().find_map(|child| detach(child, id))
}

fn create(
    tree: &mut Tree,
    parent: &BookmarkId,
    index: u32,
    title: &str,
    url: Option<&str>,
) -> Option<BookmarkId> {
    tree.next += 1;
    let id = tree.next.to_string();
    let folder = find_mut(&mut tree.root, parent).filter(|node| node.url.is_none())?;
    let at = (index as usize).min(folder.children.len());
    folder
        .children
        .insert(at, node(&id, title, url, Vec::new()));
    Some(BookmarkId::from_raw(id))
}

fn edit(tree: &mut Tree, id: &BookmarkId, change: impl FnOnce(&mut BookmarkNode)) -> Option<()> {
    change(find_mut(&mut tree.root, id)?);
    Some(())
}

fn relocate(tree: &mut Tree, id: &BookmarkId, parent: &BookmarkId, index: u32) -> Option<()> {
    let (moved, from, old) = detach(&mut tree.root, id)?;
    let mut at = index as usize;
    if from == *parent && at > old {
        at -= 1;
    }
    let folder = find_mut(&mut tree.root, parent)?;
    let at = at.min(folder.children.len());
    folder.children.insert(at, moved);
    Some(())
}

fn rejected<T>(outcome: Option<T>) -> rikka_web::Result<T> {
    outcome.ok_or_else(|| rikka_web::Error::Rejected("no such bookmark".to_owned()))
}

impl Remote for Chrome {
    fn create(
        &self,
        parent: &BookmarkId,
        index: u32,
        title: &str,
        url: Option<&str>,
    ) -> impl Future<Output = rikka_web::Result<BookmarkId>> + Send {
        ready(rejected(
            self.apply(|tree| create(tree, parent, index, title, url)),
        ))
    }

    fn rename(
        &self,
        id: &BookmarkId,
        title: &str,
    ) -> impl Future<Output = rikka_web::Result<()>> + Send {
        ready(rejected(self.apply(|tree| {
            edit(tree, id, |node| node.title = title.to_owned())
        })))
    }

    fn set_url(
        &self,
        id: &BookmarkId,
        url: &str,
    ) -> impl Future<Output = rikka_web::Result<()>> + Send {
        ready(rejected(self.apply(|tree| {
            edit(tree, id, |node| node.url = Some(url.to_owned()))
        })))
    }

    fn move_to(
        &self,
        id: &BookmarkId,
        parent: &BookmarkId,
        index: u32,
    ) -> impl Future<Output = rikka_web::Result<()>> + Send {
        ready(rejected(
            self.apply(|tree| relocate(tree, id, parent, index)),
        ))
    }

    fn remove(&self, id: &BookmarkId) -> impl Future<Output = rikka_web::Result<()>> + Send {
        ready(rejected(
            self.apply(|tree| detach(&mut tree.root, id).map(|_| ())),
        ))
    }
}

#[derive(Default)]
pub(crate) struct Desk {
    pub(crate) mirror: Mirror,
    next: i64,
    pub(crate) changes: usize,
}

impl Desk {
    pub(crate) fn new(favorites: &[&str], spaces: &[(&str, Vec<Shape>)]) -> Desk {
        let mut desk = Desk::default();
        for title in favorites {
            desk.add_favorite(&url(title), title);
        }
        for (name, pinned) in spaces {
            let space = desk.add_space(name).expect("space");
            for shape in pinned {
                desk.build(PinnedParent::Root(space), shape);
            }
        }
        desk.changes = 0;
        desk
    }

    fn build(&mut self, parent: PinnedParent, shape: &Shape) {
        let place = place(parent);
        match shape {
            Shape::Pin(title) => {
                self.add_pin(place, &url(title), title);
            }
            Shape::Folder(name, children) => {
                let folder = self.add_folder(place, name).expect("folder");
                for child in children {
                    self.build(PinnedParent::Folder(folder), child);
                }
            }
        }
    }

    fn issue(&mut self) -> i64 {
        self.next += 1;
        self.changes += 1;
        self.next
    }

    pub(crate) fn favorite(&self, title: &str) -> FavoriteId {
        self.mirror
            .favorites
            .iter()
            .find(|favorite| favorite.title == title)
            .expect("titled favorite")
            .id
    }

    pub(crate) fn item(&self, title: &str) -> PinnedItem {
        fn search(nodes: &[MirrorNode], title: &str) -> Option<PinnedItem> {
            nodes.iter().find_map(|node| match node {
                MirrorNode::Folder { id, name, .. } if name == title => {
                    Some(PinnedItem::Folder(*id))
                }
                MirrorNode::Folder { children, .. } => search(children, title),
                MirrorNode::Pin { id, title: pin, .. } if pin == title => {
                    Some(PinnedItem::Tab(*id))
                }
                MirrorNode::Pin { .. } => None,
            })
        }
        self.mirror
            .spaces
            .iter()
            .find_map(|space| search(&space.pinned, title))
            .expect("titled pinned item")
    }

    pub(crate) fn outline(&self) -> String {
        let mut parts = vec![
            self.mirror
                .favorites
                .iter()
                .map(|favorite| favorite.title.clone())
                .collect::<Vec<_>>()
                .join(" "),
        ];
        parts.extend(
            self.mirror
                .spaces
                .iter()
                .map(|space| format!("{}: {}", space.name, local_nodes(&space.pinned))),
        );
        parts.join(" | ")
    }

    fn list(&mut self, parent: PinnedParent) -> Option<&mut Vec<MirrorNode>> {
        fn search(nodes: &mut [MirrorNode], folder: FolderId) -> Option<&mut Vec<MirrorNode>> {
            nodes.iter_mut().find_map(|node| match node {
                MirrorNode::Folder { id, children, .. } if *id == folder => Some(children),
                MirrorNode::Folder { children, .. } => search(children, folder),
                MirrorNode::Pin { .. } => None,
            })
        }
        match parent {
            PinnedParent::Root(space) => self
                .mirror
                .spaces
                .iter_mut()
                .find(|entry| entry.id == space)
                .map(|entry| &mut entry.pinned),
            PinnedParent::Folder(folder) => self
                .mirror
                .spaces
                .iter_mut()
                .find_map(|space| search(&mut space.pinned, folder)),
        }
    }

    fn insert(&mut self, place: PinnedPlace, node: MirrorNode) -> bool {
        let Some(list) = self.list(place.parent) else {
            return false;
        };
        let at = place.index.unwrap_or(list.len()).min(list.len());
        list.insert(at, node);
        true
    }

    fn take(&mut self, item: PinnedItem) -> Option<MirrorNode> {
        fn search(nodes: &mut Vec<MirrorNode>, item: PinnedItem) -> Option<MirrorNode> {
            if let Some(at) = nodes.iter().position(|node| matches(node, item)) {
                return Some(nodes.remove(at));
            }
            nodes.iter_mut().find_map(|node| match node {
                MirrorNode::Folder { children, .. } => search(children, item),
                MirrorNode::Pin { .. } => None,
            })
        }
        self.mirror
            .spaces
            .iter_mut()
            .find_map(|space| search(&mut space.pinned, item))
    }

    fn edit(&mut self, item: PinnedItem, change: impl FnOnce(&mut MirrorNode)) {
        let Some(node) = self.node_mut(item) else {
            return;
        };
        change(node);
        self.changes += 1;
    }

    fn node_mut(&mut self, item: PinnedItem) -> Option<&mut MirrorNode> {
        fn search(nodes: &mut [MirrorNode], item: PinnedItem) -> Option<&mut MirrorNode> {
            for node in nodes {
                if matches(node, item) {
                    return Some(node);
                }
                if let MirrorNode::Folder { children, .. } = node
                    && let Some(found) = search(children, item)
                {
                    return Some(found);
                }
            }
            None
        }
        self.mirror
            .spaces
            .iter_mut()
            .find_map(|space| search(&mut space.pinned, item))
    }

    pub(crate) fn rename_favorite(&mut self, title: &str, to: &str) {
        let id = self.favorite(title);
        let url = url(title);
        self.edit_favorite(id, &url, to);
    }

    pub(crate) fn move_item(&mut self, title: &str, place: PinnedPlace) {
        let item = self.item(title);
        self.move_pinned(item, place);
    }

    pub(crate) fn remove_item(&mut self, title: &str) {
        let item = self.item(title);
        self.remove_pinned(item);
    }
}

fn matches(node: &MirrorNode, item: PinnedItem) -> bool {
    match (node, item) {
        (MirrorNode::Folder { id, .. }, PinnedItem::Folder(folder)) => *id == folder,
        (MirrorNode::Pin { id, .. }, PinnedItem::Tab(pin)) => *id == pin,
        _ => false,
    }
}

fn local_nodes(nodes: &[MirrorNode]) -> String {
    nodes
        .iter()
        .map(|node| match node {
            MirrorNode::Pin { title, .. } => title.clone(),
            MirrorNode::Folder { name, children, .. } => {
                format!("{name}({})", local_nodes(children))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn place(parent: PinnedParent) -> PinnedPlace {
    match parent {
        PinnedParent::Root(space) => PinnedPlace::root(space),
        PinnedParent::Folder(folder) => PinnedPlace::folder(folder),
    }
}

pub(crate) fn url(title: &str) -> String {
    format!("https://{}.example/", title.to_lowercase())
}

pub(crate) enum Shape {
    Pin(&'static str),
    Folder(&'static str, Vec<Shape>),
}

impl LocalSide for Desk {
    fn add_favorite(&mut self, url: &str, title: &str) -> Option<FavoriteId> {
        let id = FavoriteId::from_raw(self.issue());
        self.mirror.favorites.push(MirrorFavorite {
            id,
            url: url.to_owned(),
            title: title.to_owned(),
        });
        Some(id)
    }

    fn edit_favorite(&mut self, id: FavoriteId, url: &str, title: &str) {
        self.changes += 1;
        if let Some(favorite) = self
            .mirror
            .favorites
            .iter_mut()
            .find(|favorite| favorite.id == id)
        {
            favorite.url = url.to_owned();
            favorite.title = title.to_owned();
        }
    }

    fn remove_favorite(&mut self, id: FavoriteId) {
        self.changes += 1;
        self.mirror.favorites.retain(|favorite| favorite.id != id);
    }

    fn order_favorites(&mut self, order: &[FavoriteId]) {
        self.changes += 1;
        self.mirror.favorites.sort_by_key(|favorite| {
            order
                .iter()
                .position(|id| *id == favorite.id)
                .unwrap_or(usize::MAX)
        });
    }

    fn add_space(&mut self, name: &str) -> Option<SpaceId> {
        let id = SpaceId::from_raw(self.issue());
        self.mirror.spaces.push(MirrorSpace {
            id,
            name: name.to_owned(),
            pinned: Vec::new(),
        });
        Some(id)
    }

    fn rename_space(&mut self, id: SpaceId, name: &str) {
        self.changes += 1;
        if let Some(space) = self.mirror.spaces.iter_mut().find(|space| space.id == id) {
            space.name = name.to_owned();
        }
    }

    fn add_folder(&mut self, place: PinnedPlace, name: &str) -> Option<FolderId> {
        let id = FolderId::from_raw(self.issue());
        let folder = MirrorNode::Folder {
            id,
            name: name.to_owned(),
            children: Vec::new(),
        };
        self.insert(place, folder).then_some(id)
    }

    fn rename_folder(&mut self, id: FolderId, name: &str) {
        self.edit(PinnedItem::Folder(id), |node| {
            if let MirrorNode::Folder { name: kept, .. } = node {
                *kept = name.to_owned();
            }
        });
    }

    fn add_pin(&mut self, place: PinnedPlace, url: &str, title: &str) -> Option<PinnedTabId> {
        let id = PinnedTabId::from_raw(self.issue());
        let pin = MirrorNode::Pin {
            id,
            url: url.to_owned(),
            title: title.to_owned(),
        };
        self.insert(place, pin).then_some(id)
    }

    fn edit_pin(&mut self, id: PinnedTabId, url: &str, title: &str) {
        self.edit(PinnedItem::Tab(id), |node| {
            if let MirrorNode::Pin {
                url: kept_url,
                title: kept_title,
                ..
            } = node
            {
                *kept_url = url.to_owned();
                *kept_title = title.to_owned();
            }
        });
    }

    fn move_pinned(&mut self, item: PinnedItem, place: PinnedPlace) {
        self.changes += 1;
        if let Some(node) = self.take(item) {
            self.insert(place, node);
        }
    }

    fn remove_pinned(&mut self, item: PinnedItem) {
        self.changes += 1;
        self.take(item);
    }
}

pub(crate) fn finish<T>(future: impl Future<Output = T>) -> T {
    let mut future = pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("fake bookmark calls finish at once"),
    }
}

pub(crate) fn sync(chrome: &Chrome, desk: &mut Desk, synced: &mut Option<BookmarkSync>) {
    let plan = Plan::new(&chrome.tree(), &desk.mirror, synced.as_ref()).expect("plan");
    let pending = plan.apply_local(desk);
    *synced = Some(finish(pending.apply_remote(chrome)).expect("remote changes apply"));
}
