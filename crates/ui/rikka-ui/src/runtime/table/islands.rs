use rikka_renderer::GroupId;

use crate::runtime::layout::{Layouts, Node};

pub(crate) const NO_ISLAND: u32 = u32::MAX;

pub(crate) const MERGED_BELOW: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bound {
    Open,
    Always,
    Fixed,
}

#[derive(Debug)]
pub(crate) struct Island {
    owner: Option<Node>,
    parent: u32,
    nodes: Vec<Node>,
    end: Vec<u32>,
    nested: Vec<u32>,
    pub(crate) group: Option<GroupId>,
    packed: u64,
    stale: bool,
    live: bool,
}

impl Island {
    fn new(owner: Option<Node>, parent: u32) -> Self {
        Island {
            owner,
            parent,
            nodes: Vec::new(),
            end: Vec::new(),
            nested: Vec::new(),
            group: None,
            packed: 0,
            stale: true,
            live: true,
        }
    }

    fn reopen(&mut self, owner: Option<Node>, parent: u32) {
        self.owner = owner;
        self.parent = parent;
        self.nodes.clear();
        self.end.clear();
        self.nested.clear();
        self.group = None;
        self.packed = 0;
        self.stale = true;
        self.live = true;
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DissolvedIsland {
    pub(crate) group: GroupId,
    pub(crate) owner: Option<Node>,
}

#[derive(Debug, Default)]
pub(crate) struct Islands {
    list: Vec<Island>,
    free: Vec<u32>,
    owned: Vec<u32>,
    seat: Vec<u32>,
    stale: Vec<u32>,
    packs: u64,
    root: Option<(u32, Node)>,
    dissolved: Vec<DissolvedIsland>,
    spare: Vec<u32>,
    doomed: Vec<u32>,
}

impl Islands {
    pub(crate) fn root(&self) -> Option<u32> {
        self.root.map(|(island, _)| island)
    }

    pub(crate) fn island(&self, index: u32) -> &Island {
        &self.list[index as usize]
    }

    pub(crate) fn group_mut(&mut self, index: u32) -> &mut Option<GroupId> {
        &mut self.list[index as usize].group
    }

    pub(crate) fn owned(&self, node: Node) -> Option<u32> {
        self.owned
            .get(node.index())
            .copied()
            .filter(|island| *island != NO_ISLAND)
    }

    pub(crate) fn take_order(&mut self, index: u32) -> (Vec<Node>, Vec<u32>) {
        let island = &mut self.list[index as usize];
        (
            std::mem::take(&mut island.nodes),
            std::mem::take(&mut island.end),
        )
    }

    pub(crate) fn restore_order(&mut self, index: u32, nodes: Vec<Node>, end: Vec<u32>) {
        let island = &mut self.list[index as usize];
        island.nodes = nodes;
        island.end = end;
    }

    pub(crate) fn take_dissolved(&mut self) -> Vec<DissolvedIsland> {
        std::mem::take(&mut self.dissolved)
    }

    fn grow(&mut self, node: Node) {
        let needed_len = node.index() + 1;
        if self.owned.len() < needed_len {
            self.owned.resize(needed_len, NO_ISLAND);
            self.seat.resize(needed_len, NO_ISLAND);
        }
    }

    pub(crate) fn insert(&mut self, node: Node) {
        self.grow(node);
        self.owned[node.index()] = NO_ISLAND;
        self.seat[node.index()] = NO_ISLAND;
    }

    pub(crate) fn set_root(&mut self, node: Node) {
        if let Some((island, root_node)) = self.root {
            if root_node == node {
                self.spoil(island);
                return;
            }
            self.dissolve_all();
        }
        let island = self.open(None, NO_ISLAND);
        self.root = Some((island, node));
    }

    pub(crate) fn mark(&mut self, node: Node) {
        let island = match self.owned(node) {
            Some(island) => island,
            None => match self.seat.get(node.index()) {
                Some(&island) if island != NO_ISLAND => island,
                _ => return,
            },
        };
        self.spoil(island);
    }

    pub(crate) fn mark_around(&mut self, node: Node, layouts: &Layouts) {
        match layouts.parent(node) {
            Some(parent) => self.mark(parent),
            None => self.mark(node),
        }
    }

    pub(crate) fn is_stale(&self) -> bool {
        !self.stale.is_empty()
    }

    pub(crate) fn refresh(&mut self, layouts: &Layouts, bound: impl Fn(Node) -> Bound) {
        while let Some(index) = self.stale.pop() {
            let island = &self.list[index as usize];
            if island.live && island.stale {
                self.repack(index, layouts, &bound);
            }
        }
    }

    fn spoil(&mut self, index: u32) {
        let island = &mut self.list[index as usize];
        if !island.stale {
            island.stale = true;
            self.stale.push(index);
        }
    }

    fn open(&mut self, owner: Option<Node>, parent: u32) -> u32 {
        let index = match self.free.pop() {
            Some(index) => {
                self.list[index as usize].reopen(owner, parent);
                index
            }
            None => {
                self.list.push(Island::new(owner, parent));
                (self.list.len() - 1) as u32
            }
        };
        self.stale.push(index);
        index
    }

    fn repack(&mut self, index: u32, layouts: &Layouts, bound: &impl Fn(Node) -> Bound) {
        self.packs += 1;
        let island = &mut self.list[index as usize];
        island.stale = false;
        let mut root_node = std::mem::take(&mut self.spare);
        root_node.clear();
        root_node.extend_from_slice(&island.nested);
        let mut packing = Packing {
            index,
            nodes: std::mem::take(&mut island.nodes),
            end: std::mem::take(&mut island.end),
            nested: std::mem::take(&mut island.nested),
        };
        packing.nodes.clear();
        packing.end.clear();
        packing.nested.clear();
        match (island.owner, self.root) {
            (Some(owner), _) => {
                for &entry in layouts.children(owner) {
                    self.pack(&mut packing, entry, layouts, bound);
                }
            }
            (None, Some((_, root))) => self.pack(&mut packing, root, layouts, bound),
            (None, None) => {}
        }
        let island = &mut self.list[index as usize];
        island.nodes = packing.nodes;
        island.end = packing.end;
        island.nested = packing.nested;
        for &nested in &root_node {
            let left = &self.list[nested as usize];
            if left.live && left.parent == index && left.packed != self.packs {
                self.dissolve(nested);
            }
        }
        self.spare = root_node;
    }

    fn pack(
        &mut self,
        packing: &mut Packing,
        node: Node,
        layouts: &Layouts,
        bound: &impl Fn(Node) -> Bound,
    ) {
        self.grow(node);
        let position = packing.nodes.len();
        packing.nodes.push(node);
        packing.end.push(position as u32 + 1);
        self.seat[node.index()] = packing.index;
        let owns = match bound(node) {
            Bound::Always => true,
            Bound::Fixed => at_least(layouts, node, MERGED_BELOW),
            Bound::Open => false,
        };
        if owns {
            let island = match self.owned(node) {
                Some(island) if self.list[island as usize].live => island,
                _ => {
                    let island = self.open(Some(node), packing.index);
                    self.owned[node.index()] = island;
                    island
                }
            };
            let nested = &mut self.list[island as usize];
            nested.parent = packing.index;
            nested.packed = self.packs;
            packing.nested.push(island);
            return;
        }
        self.owned[node.index()] = NO_ISLAND;
        for &child in layouts.children(node) {
            self.pack(packing, child, layouts, bound);
        }
        packing.end[position] = packing.nodes.len() as u32;
    }

    fn dissolve(&mut self, index: u32) {
        let mut doomed = std::mem::take(&mut self.doomed);
        doomed.push(index);
        while let Some(index) = doomed.pop() {
            let island = &mut self.list[index as usize];
            island.live = false;
            island.nodes.clear();
            island.end.clear();
            let owner = island.owner;
            if let Some(group) = island.group.take() {
                self.dissolved.push(DissolvedIsland { group, owner });
            }
            if let Some(owner) = owner
                && self.owned.get(owner.index()) == Some(&index)
            {
                self.owned[owner.index()] = NO_ISLAND;
            }
            self.free.push(index);
            let island = &self.list[index as usize];
            doomed.extend(island.nested.iter().copied().filter(|&below| {
                let below = &self.list[below as usize];
                below.live && below.parent == index
            }));
            self.list[index as usize].nested.clear();
        }
        self.doomed = doomed;
    }

    fn dissolve_all(&mut self) {
        for index in 0..self.list.len() as u32 {
            if self.list[index as usize].live {
                self.dissolve(index);
            }
        }
        self.root = None;
    }
}

struct Packing {
    index: u32,
    nodes: Vec<Node>,
    end: Vec<u32>,
    nested: Vec<u32>,
}

fn at_least(layouts: &Layouts, node: Node, needed_len: usize) -> bool {
    fn count(layouts: &Layouts, node: Node, left: &mut usize) {
        for &child in layouts.children(node) {
            if *left == 0 {
                return;
            }
            *left -= 1;
            count(layouts, child, left);
        }
    }
    let mut left = needed_len.saturating_sub(1);
    count(layouts, node, &mut left);
    left == 0
}
