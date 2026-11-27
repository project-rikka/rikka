use std::collections::HashMap;

use rikka_ui::Observers;

use crate::ids::{FolderId, PinnedItem, PinnedTabId, SpaceId};
use crate::saved::Saved;

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Folder(Folder),
    Pin(Pin),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Folder {
    pub id: FolderId,
    pub name: String,
    pub open: bool,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pin {
    pub id: PinnedTabId,
    pub saved: Saved,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PinPlace {
    pub parent: Option<FolderId>,
    pub index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    pub item: PinnedItem,
    pub depth: usize,
    pub open: bool,
    pub place: PinPlace,
}

impl Line {
    pub fn folder(&self) -> Option<FolderId> {
        match self.item {
            PinnedItem::Folder(folder) => Some(folder),
            PinnedItem::Tab(_) => None,
        }
    }
}

impl Node {
    pub fn item(&self) -> PinnedItem {
        match self {
            Node::Folder(folder) => PinnedItem::Folder(folder.id),
            Node::Pin(pin) => PinnedItem::Tab(pin.id),
        }
    }

    fn holds(&self, item: PinnedItem) -> bool {
        match self {
            Node::Folder(folder) => folder
                .children
                .iter()
                .any(|child| child.item() == item || child.holds(item)),
            Node::Pin(_) => false,
        }
    }

    pub fn pins(&self, into: &mut Vec<PinnedTabId>) {
        match self {
            Node::Folder(folder) => folder.children.iter().for_each(|child| child.pins(into)),
            Node::Pin(pin) => into.push(pin.id),
        }
    }

    pub fn from_store(stored: browser_store::PinnedNode) -> Node {
        match stored {
            browser_store::PinnedNode::Folder { folder, children } => Node::Folder(Folder {
                id: folder.id,
                name: folder.name,
                open: folder.expanded,
                children: children.into_iter().map(Node::from_store).collect(),
            }),
            browser_store::PinnedNode::Tab(tab) => Node::Pin(Pin {
                id: tab.id,
                saved: Saved::new(tab.url, tab.title.unwrap_or_default()),
            }),
        }
    }

    pub fn saved_mut(&mut self, visit: &mut impl FnMut(&mut Saved)) {
        match self {
            Node::Folder(folder) => folder
                .children
                .iter_mut()
                .for_each(|child| child.saved_mut(visit)),
            Node::Pin(pin) => visit(&mut pin.saved),
        }
    }
}

#[derive(Debug, Default)]
pub struct Pinned {
    by_space: HashMap<SpaceId, Vec<Node>>,
    observers: Observers,
}

impl Pinned {
    pub fn new(by_space: HashMap<SpaceId, Vec<Node>>) -> Pinned {
        Pinned {
            by_space,
            observers: Observers::new(),
        }
    }

    pub fn tree(&self, space: SpaceId) -> &[Node] {
        self.observers.read();
        self.by_space.get(&space).map_or(&[], Vec::as_slice)
    }

    pub fn lines(&self, space: SpaceId) -> Vec<Line> {
        let mut lines = Vec::new();
        walk(self.tree(space), None, 0, &mut lines);
        lines
    }

    pub fn pins(&self, space: SpaceId) -> Vec<PinnedTabId> {
        let mut pins = Vec::new();
        self.tree(space)
            .iter()
            .for_each(|node| node.pins(&mut pins));
        pins
    }

    pub fn find(&self, item: PinnedItem) -> Option<(SpaceId, &Node)> {
        self.observers.read();
        self.by_space
            .iter()
            .find_map(|(space, nodes)| find(nodes, item).map(|node| (*space, node)))
    }

    pub fn pin(&self, id: PinnedTabId) -> Option<(SpaceId, &Pin)> {
        match self.find(PinnedItem::Tab(id))? {
            (space, Node::Pin(pin)) => Some((space, pin)),
            (_, Node::Folder(_)) => None,
        }
    }

    pub fn folder(&self, id: FolderId) -> Option<&Folder> {
        match self.find(PinnedItem::Folder(id))? {
            (_, Node::Folder(folder)) => Some(folder),
            (_, Node::Pin(_)) => None,
        }
    }

    pub fn place_of(&self, item: PinnedItem) -> Option<(SpaceId, PinPlace)> {
        self.by_space
            .iter()
            .find_map(|(space, nodes)| place_of(nodes, None, item).map(|place| (*space, place)))
    }

    pub fn insert(&mut self, space: SpaceId, place: PinPlace, node: Node) -> bool {
        let nodes = self.by_space.entry(space).or_default();
        let Some(siblings) = children_mut(nodes, place.parent) else {
            return false;
        };
        siblings.insert(place.index.min(siblings.len()), node);
        self.observers.changed();
        true
    }

    pub fn remove(&mut self, item: PinnedItem) -> Option<(SpaceId, PinPlace, Node)> {
        let (space, place) = self.place_of(item)?;
        let nodes = self.by_space.get_mut(&space)?;
        let siblings = children_mut(nodes, place.parent)?;
        let node = siblings.remove(place.index);
        self.observers.changed();
        Some((space, place, node))
    }

    pub fn place(&mut self, item: PinnedItem, space: SpaceId, place: PinPlace) -> bool {
        let moved_into_itself = place.parent.is_some_and(|parent| {
            PinnedItem::Folder(parent) == item
                || self
                    .find(item)
                    .is_some_and(|(_, node)| node.holds(PinnedItem::Folder(parent)))
        });
        if moved_into_itself {
            return false;
        }
        let Some((from_space, from, node)) = self.remove(item) else {
            return false;
        };
        let place = match from_space == space && from.parent == place.parent {
            true if place.index > from.index => PinPlace {
                index: place.index - 1,
                ..place
            },
            _ => place,
        };
        if self.insert(space, place, node.clone()) {
            return true;
        }
        self.insert(from_space, from, node);
        false
    }

    pub fn edit_folder(&mut self, id: FolderId, edit: impl FnOnce(&mut Folder)) -> bool {
        let entry = self
            .by_space
            .values_mut()
            .find_map(|nodes| find_mut(nodes, PinnedItem::Folder(id)));
        let Some(Node::Folder(folder)) = entry else {
            return false;
        };
        let before = (folder.name.clone(), folder.open);
        edit(folder);
        let changed = (folder.name.clone(), folder.open) != before;
        if changed {
            self.observers.changed();
        }
        changed
    }

    pub fn retitle(&mut self, id: PinnedTabId, url: &str, title: &str) {
        let entry = self
            .by_space
            .values_mut()
            .find_map(|nodes| find_mut(nodes, PinnedItem::Tab(id)));
        let Some(Node::Pin(pin)) = entry else {
            return;
        };
        if pin.saved.url != url {
            pin.saved.url = url.to_owned();
            pin.saved.favicon = None;
        }
        pin.saved.title = title.to_owned();
        self.observers.changed();
    }

    pub fn remember(&mut self, id: PinnedTabId, saved: &Saved) {
        let entry = self
            .by_space
            .values_mut()
            .find_map(|nodes| find_mut(nodes, PinnedItem::Tab(id)));
        if let Some(Node::Pin(pin)) = entry
            && pin.saved.favicon.is_none()
            && saved.favicon.is_some()
        {
            pin.saved.favicon = saved.favicon.clone();
            self.observers.changed();
        }
    }

    pub fn remove_space(&mut self, space: SpaceId) -> Vec<Node> {
        let removed = self.by_space.remove(&space).unwrap_or_default();
        self.observers.changed();
        removed
    }
}

fn walk(nodes: &[Node], parent: Option<FolderId>, depth: usize, lines: &mut Vec<Line>) {
    for (index, node) in nodes.iter().enumerate() {
        let place = PinPlace { parent, index };
        match node {
            Node::Pin(pin) => lines.push(Line {
                item: PinnedItem::Tab(pin.id),
                depth,
                open: false,
                place,
            }),
            Node::Folder(folder) => {
                lines.push(Line {
                    item: PinnedItem::Folder(folder.id),
                    depth,
                    open: folder.open,
                    place,
                });
                if folder.open {
                    walk(&folder.children, Some(folder.id), depth + 1, lines);
                }
            }
        }
    }
}

fn find(nodes: &[Node], item: PinnedItem) -> Option<&Node> {
    nodes.iter().find_map(|node| match node {
        _ if node.item() == item => Some(node),
        Node::Folder(folder) => find(&folder.children, item),
        Node::Pin(_) => None,
    })
}

fn find_mut(nodes: &mut [Node], item: PinnedItem) -> Option<&mut Node> {
    for node in nodes {
        if node.item() == item {
            return Some(node);
        }
        if let Node::Folder(folder) = node
            && let Some(nested) = find_mut(&mut folder.children, item)
        {
            return Some(nested);
        }
    }
    None
}

fn children_mut(nodes: &mut Vec<Node>, parent: Option<FolderId>) -> Option<&mut Vec<Node>> {
    let Some(parent) = parent else {
        return Some(nodes);
    };
    match find_mut(nodes, PinnedItem::Folder(parent))? {
        Node::Folder(folder) => Some(&mut folder.children),
        Node::Pin(_) => None,
    }
}

fn place_of(nodes: &[Node], parent: Option<FolderId>, item: PinnedItem) -> Option<PinPlace> {
    for (index, node) in nodes.iter().enumerate() {
        if node.item() == item {
            return Some(PinPlace { parent, index });
        }
        if let Node::Folder(folder) = node
            && let Some(place) = place_of(&folder.children, Some(folder.id), item)
        {
            return Some(place);
        }
    }
    None
}
