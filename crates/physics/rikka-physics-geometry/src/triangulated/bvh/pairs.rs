use rikka_physics_math::Aabb;

use super::bounds4::Bounds4;
use super::items::lane_mask;
use super::node::Child;
use super::stack::Stack;
use super::traverse::Nodes;
use super::tree::Bvh4;

const ALL_SLOTS: u32 = 0b1111;

#[derive(Clone, Copy, Debug, Default)]
enum Work {
    #[default]
    Empty,
    Within(u32),
    Nodes(u32, u32),
    LeafNode(u32, u32),
    NodeLeaf(u32, u32),
    LeafWithin(u32),
    Leaves(u32, u32),
}

impl Work {
    #[inline]
    fn is_terminal(&self) -> bool {
        matches!(self, Work::Empty | Work::LeafWithin(_) | Work::Leaves(..))
    }

    #[inline]
    fn rank(&self) -> u8 {
        match self {
            Work::Within(_) => 0,
            Work::Nodes(..) => 1,
            Work::LeafNode(..) | Work::NodeLeaf(..) => 2,
            Work::LeafWithin(_) | Work::Leaves(..) => 3,
            Work::Empty => 4,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PairTask {
    work: Work,
    pub(super) moved_only: bool,
}

#[derive(Clone, Copy)]
enum Side {
    Node(u32),
    Leaf(u32),
}

#[derive(Clone, Copy)]
pub(crate) struct Pairs<'a> {
    a: &'a Bvh4,
    b: &'a Bvh4,
    moved_only: bool,
}

#[inline]
fn bit(slot: usize) -> u32 {
    1 << slot
}

#[inline]
fn reference(node: u32, slot: usize) -> u32 {
    node << 2 | slot as u32
}

#[inline]
fn for_each_bit(mut mask: u32, mut visit: impl FnMut(usize) -> bool) -> bool {
    while mask != 0 {
        let index = mask.trailing_zeros() as usize;
        mask &= mask - 1;
        if !visit(index) {
            return false;
        }
    }
    true
}

#[inline]
fn pair(first: Side, second: Side) -> Work {
    match (first, second) {
        (Side::Node(a), Side::Node(b)) => Work::Nodes(a, b),
        (Side::Leaf(a), Side::Node(b)) => Work::LeafNode(a, b),
        (Side::Node(a), Side::Leaf(b)) => Work::NodeLeaf(a, b),
        (Side::Leaf(a), Side::Leaf(b)) => Work::Leaves(a, b),
    }
}

impl<'a> Pairs<'a> {
    pub(crate) fn self_pairs(tree: &'a Bvh4, moved_only: bool) -> Self {
        Self {
            a: tree,
            b: tree,
            moved_only,
        }
    }

    pub(crate) fn cross_pairs(a: &'a Bvh4, b: &'a Bvh4) -> Self {
        Self {
            a,
            b,
            moved_only: false,
        }
    }

    pub(crate) fn self_root(&self) -> Option<PairTask> {
        let root = self.a.root()?;
        if self.moved_only && self.a.moved.slots(root) == 0 {
            return None;
        }
        Some(self.task(Work::Within(root)))
    }

    pub(crate) fn cross_root(&self) -> Option<PairTask> {
        Some(self.task(Work::Nodes(self.a.root()?, self.b.root()?)))
    }

    #[inline]
    fn task(&self, work: Work) -> PairTask {
        PairTask {
            work,
            moved_only: self.moved_only,
        }
    }

    pub(crate) fn tasks(&self, target: usize, tasks: &mut Vec<PairTask>) {
        tasks.clear();
        let Some(root) = self.self_root() else {
            return;
        };
        tasks.push(root);
        while tasks.len() < target {
            let count = tasks.len();
            let mut expanded = false;
            for index in 0..count {
                let task = tasks[index];
                if task.work.is_terminal() {
                    tasks.push(task);
                } else {
                    expanded = true;
                    self.expand(task.work, &mut |work| {
                        tasks.push(self.task(work));
                        true
                    });
                }
            }
            tasks.drain(..count);
            if !expanded {
                break;
            }
        }
        tasks.sort_by_key(|task| task.work.rank());
    }

    pub(crate) fn run(&self, task: PairTask, visit: &mut impl FnMut(u32, u32) -> bool) -> bool {
        if task.work.is_terminal() {
            return self.emit(task.work, visit);
        }
        let mut stack = Stack::<Work>::new();
        stack.push(task.work);
        while let Some(work) = stack.pop() {
            let mut sink = |work: Work| {
                if work.is_terminal() {
                    self.emit(work, visit)
                } else {
                    stack.push(work);
                    true
                }
            };
            if !self.expand(work, &mut sink) {
                return false;
            }
        }
        true
    }

    #[inline]
    fn moved_slots(&self, tree: &Bvh4, node: u32) -> u32 {
        if self.moved_only {
            tree.moved.slots(node)
        } else {
            ALL_SLOTS
        }
    }

    #[inline]
    fn side(tree: &Bvh4, node: u32, slot: usize) -> Side {
        match tree.child(node, slot) {
            Child::Node(child) => Side::Node(child),
            _ => Side::Leaf(reference(node, slot)),
        }
    }

    #[inline]
    fn leaf_bounds(tree: &Bvh4, leaf: u32) -> Aabb {
        tree.node(leaf >> 2).bounds((leaf & 3) as usize)
    }

    #[inline]
    fn leaf_range(tree: &Bvh4, leaf: u32) -> (u32, u32) {
        match tree.child(leaf >> 2, (leaf & 3) as usize) {
            Child::Leaf { start, count } => (start, count),
            _ => (0, 0),
        }
    }

    fn expand(&self, work: Work, sink: &mut impl FnMut(Work) -> bool) -> bool {
        match work {
            Work::Within(node) => self.expand_within(node, sink),
            Work::Nodes(a, b) => self.expand_nodes(a, b, sink),
            Work::LeafNode(leaf, node) => self.expand_leaf_node(leaf, node, sink),
            Work::NodeLeaf(node, leaf) => self.expand_node_leaf(node, leaf, sink),
            Work::Empty | Work::LeafWithin(_) | Work::Leaves(..) => true,
        }
    }

    fn expand_within(&self, node: u32, sink: &mut impl FnMut(Work) -> bool) -> bool {
        let tree = self.a;
        let bounds = tree.load(node, &());
        let occupied = tree.occupied(node);
        let moved = self.moved_slots(tree, node) & occupied;
        for_each_bit(occupied, |first| {
            let side = Self::side(tree, node, first);
            let own = moved & bit(first) != 0;
            if own {
                let proceed = match side {
                    Side::Node(child) => sink(Work::Within(child)),
                    Side::Leaf(leaf) => {
                        let (_, count) = Self::leaf_range(tree, leaf);
                        count < 2 || sink(Work::LeafWithin(leaf))
                    }
                };
                if !proceed {
                    return false;
                }
            }
            let later = !((2u32 << first) - 1);
            let mut mask = bounds.overlaps(&bounds.lane(first)) & occupied & later;
            if !own {
                mask &= moved;
            }
            for_each_bit(mask, |second| {
                sink(pair(side, Self::side(tree, node, second)))
            })
        })
    }

    fn expand_nodes(&self, node_a: u32, node_b: u32, sink: &mut impl FnMut(Work) -> bool) -> bool {
        let (a, b) = (self.a, self.b);
        let bounds_a = a.load(node_a, &());
        let bounds_b = b.load(node_b, &());
        let occupied_b = b.occupied(node_b);
        let moved_a = self.moved_slots(a, node_a);
        let moved_b = self.moved_slots(b, node_b) & occupied_b;
        for_each_bit(a.occupied(node_a), |first| {
            let mut mask = bounds_b.overlaps(&bounds_a.lane(first)) & occupied_b;
            if moved_a & bit(first) == 0 {
                mask &= moved_b;
            }
            if mask == 0 {
                return true;
            }
            let side = Self::side(a, node_a, first);
            for_each_bit(mask, |second| {
                sink(pair(side, Self::side(b, node_b, second)))
            })
        })
    }

    #[inline]
    fn leaf_moved(&self, tree: &Bvh4, leaf: u32) -> bool {
        self.moved_slots(tree, leaf >> 2) & bit((leaf & 3) as usize) != 0
    }

    #[inline]
    fn child_mask(
        &self,
        tree: &Bvh4,
        node: u32,
        bounds: &Bounds4,
        leaf: &Aabb,
        moved: bool,
    ) -> u32 {
        let occupied = tree.occupied(node);
        let mask = bounds.overlaps(leaf) & occupied;
        if moved {
            mask
        } else {
            mask & self.moved_slots(tree, node)
        }
    }

    fn expand_leaf_node(&self, leaf: u32, node: u32, sink: &mut impl FnMut(Work) -> bool) -> bool {
        let (a, b) = (self.a, self.b);
        let bounds = b.load(node, &());
        let mask = self.child_mask(
            b,
            node,
            &bounds,
            &Self::leaf_bounds(a, leaf),
            self.leaf_moved(a, leaf),
        );
        for_each_bit(mask, |second| {
            sink(pair(Side::Leaf(leaf), Self::side(b, node, second)))
        })
    }

    fn expand_node_leaf(&self, node: u32, leaf: u32, sink: &mut impl FnMut(Work) -> bool) -> bool {
        let (a, b) = (self.a, self.b);
        let bounds = a.load(node, &());
        let mask = self.child_mask(
            a,
            node,
            &bounds,
            &Self::leaf_bounds(b, leaf),
            self.leaf_moved(b, leaf),
        );
        for_each_bit(mask, |first| {
            sink(pair(Self::side(a, node, first), Side::Leaf(leaf)))
        })
    }

    #[inline]
    fn moved_lanes(&self, tree: &Bvh4, start: u32, count: u32) -> u32 {
        if self.moved_only {
            tree.moved.positions(start, count)
        } else {
            ALL_SLOTS
        }
    }

    fn emit(&self, work: Work, visit: &mut impl FnMut(u32, u32) -> bool) -> bool {
        match work {
            Work::LeafWithin(leaf) => {
                let range = Self::leaf_range(self.a, leaf);
                self.emit_blocks(self.a, range, self.a, range, true, visit)
            }
            Work::Leaves(leaf_a, leaf_b) => self.emit_blocks(
                self.a,
                Self::leaf_range(self.a, leaf_a),
                self.b,
                Self::leaf_range(self.b, leaf_b),
                false,
                visit,
            ),
            _ => true,
        }
    }

    fn emit_blocks(
        &self,
        a: &Bvh4,
        (start_a, count_a): (u32, u32),
        b: &Bvh4,
        (start_b, count_b): (u32, u32),
        within: bool,
        visit: &mut impl FnMut(u32, u32) -> bool,
    ) -> bool {
        let (items_a, items_b) = (a.items.as_slice(), b.items.as_slice());
        let (end_a, end_b) = (start_a + count_a, start_b + count_b);
        let mut chunk_a = start_a;
        while chunk_a < end_a {
            let lanes_a = a.item_bounds.lanes(chunk_a as usize);
            let owned = self.moved_lanes(a, chunk_a, end_a - chunk_a);
            let count = (end_a - chunk_a).min(4) as usize;
            let mut chunk_b = if within { chunk_a } else { start_b };
            while chunk_b < end_b {
                let lanes_b = b.item_bounds.lanes(chunk_b as usize);
                let valid = lane_mask(end_b - chunk_b);
                let moved = self.moved_lanes(b, chunk_b, end_b - chunk_b);
                for lane_a in 0..count {
                    let mut mask = lanes_b.overlaps(&lanes_a.lane(lane_a)) & valid;
                    if within && chunk_b == chunk_a {
                        mask &= !((2u32 << lane_a) - 1);
                    }
                    if owned & bit(lane_a) == 0 {
                        mask &= moved;
                    }
                    let first = items_a[chunk_a as usize + lane_a];
                    if !for_each_bit(mask, |lane_b| {
                        visit(first, items_b[chunk_b as usize + lane_b])
                    }) {
                        return false;
                    }
                }
                chunk_b += 4;
            }
            chunk_a += 4;
        }
        true
    }
}
