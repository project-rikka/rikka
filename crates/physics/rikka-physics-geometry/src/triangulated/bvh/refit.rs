use rikka_sync::Mutex;

use rikka_physics_core::{Executor, ExecutorExt};
use rikka_physics_math::Aabb;

use super::build::{Builder, NodeAlloc};
use super::items::ItemSlices;
use super::node::{Child, EMPTY_CHILD, Node4};
use super::stack::Stack;
use super::tree::{Bvh4, decode_location};

const REFIT_BLOCK: usize = 1024;
const REFIT_BLOCKS: usize = 64;

struct Reusing<'a> {
    nodes: &'a mut Vec<Node4>,
    free: &'a [u32],
    used: usize,
}

impl NodeAlloc for Reusing<'_> {
    fn allocate(&mut self) -> u32 {
        match self.free.get(self.used) {
            Some(&index) => {
                self.used += 1;
                index
            }
            None => self.nodes.allocate(),
        }
    }

    fn node_mut(&mut self, index: u32) -> &mut Node4 {
        &mut self.nodes[index as usize]
    }
}

impl Bvh4 {
    pub fn refit(&mut self, bounds: &[Aabb]) {
        if self.nodes.is_empty() {
            return;
        }
        self.item_bounds.update(&self.items, 0, bounds);
        self.refit_nodes();
    }

    pub fn refit_on(&mut self, bounds: &[Aabb], executor: &dyn Executor) {
        if self.nodes.is_empty() {
            return;
        }
        let len = self.items.len();
        let width = REFIT_BLOCK.max(len.div_ceil(REFIT_BLOCKS));
        let blocks = len.div_ceil(width);
        if executor.workers() < 2 || blocks < 2 {
            self.refit(bounds);
            return;
        }
        {
            let items = self.items.as_slice();
            let mut chunks = self.item_bounds.chunks(len, width);
            let slots: [Mutex<Option<ItemSlices<'_>>>; REFIT_BLOCKS] =
                std::array::from_fn(|_| Mutex::new(chunks.next()));
            executor.for_each(blocks as u32, |block| {
                let index = block.index as usize;
                let mut slot = slots[index].lock();
                if let Some(slices) = slot.as_mut() {
                    let start = index * width;
                    slices.fill(&items[start..(start + width).min(len)], bounds);
                }
            });
        }
        self.refit_nodes();
    }

    fn refit_nodes(&mut self) {
        for node in (0..self.nodes.len()).rev() {
            for slot in 0..4 {
                let refit = match self.nodes[node].child(slot) {
                    Child::Empty => continue,
                    Child::Node(child) => self.nodes[child as usize].union(),
                    Child::Leaf { start, count } => self.item_bounds.union(start, count),
                };
                self.nodes[node].set_bounds(slot, refit);
            }
        }
    }

    pub fn rebuild_subtree(&mut self, node: u32, bounds: &[Aabb]) {
        self.moved.clear();
        let mut start = u32::MAX;
        let mut count = 0;
        let mut stack = Stack::new();
        stack.push(node);
        while let Some(current) = stack.pop() {
            for slot in 0..4 {
                match self.nodes[current as usize].child(slot) {
                    Child::Empty => {}
                    Child::Node(child) => {
                        stack.push(child);
                        self.free.push(child);
                    }
                    Child::Leaf {
                        start: leaf,
                        count: leaf_count,
                    } => {
                        start = start.min(leaf);
                        count += leaf_count;
                    }
                }
            }
        }
        for &freed in &self.free {
            self.nodes[freed as usize] = Node4::EMPTY;
        }
        self.free.sort_unstable();
        self.free.dedup();
        if count == 0 {
            return;
        }
        let range = start as usize..(start + count) as usize;
        let builder = Builder::new(bounds, self.leaf_size, self.bins);
        let first = self.free.partition_point(|&index| index <= node);
        let mut alloc = Reusing {
            nodes: &mut self.nodes,
            free: &self.free[first..],
            used: 0,
        };
        builder.build(
            &mut self.items[range.clone()],
            start,
            node,
            &mut alloc,
            None,
            &mut Vec::new(),
        );
        let used = alloc.used;
        self.free.drain(first..first + used);
        self.item_bounds
            .update(&self.items[range], start as usize, bounds);
        self.parents.resize(self.nodes.len(), EMPTY_CHILD);
        let parent = self.parents[node as usize];
        self.link(node, parent);
        let mut current = decode_location(parent);
        let mut child = node;
        while let Some(location) = current {
            let union = self.nodes[child as usize].union();
            self.nodes[location.node as usize].set_bounds(location.slot as usize, union);
            child = location.node;
            current = decode_location(self.parents[location.node as usize]);
        }
    }

    pub(super) fn link(&mut self, node: u32, parent: u32) {
        let mut stack = Stack::new();
        stack.push((node, parent));
        while let Some((node, parent)) = stack.pop() {
            self.parents[node as usize] = parent;
            for slot in 0..4 {
                let location = node << 2 | slot as u32;
                match self.nodes[node as usize].child(slot) {
                    Child::Empty => {}
                    Child::Node(child) => stack.push((child, location)),
                    Child::Leaf { start, count } => {
                        for position in start..start + count {
                            self.leaves[position as usize] = location;
                            self.positions
                                .insert(self.items[position as usize], position);
                        }
                    }
                }
            }
        }
    }

    pub fn mark_moved(&mut self, items: impl IntoIterator<Item = u32>) {
        if self.nodes.is_empty() {
            return;
        }
        self.moved.fit(self.nodes.len(), self.items.len());
        for item in items {
            if let Some(position) = self.positions.get(item) {
                self.moved.mark_position(position);
            }
        }
        let pending = std::mem::take(&mut self.moved.pending);
        if 4 * pending.len() < self.nodes.len() {
            for &position in &pending {
                let mut current = decode_location(self.leaves[position as usize]);
                while let Some(at) = current {
                    if !self.moved.mark_slot(at.node, at.slot) {
                        break;
                    }
                    current = decode_location(self.parents[at.node as usize]);
                }
            }
        } else {
            self.mark_nodes();
        }
        self.moved.pending = pending;
        self.moved.pending.clear();
    }

    fn mark_nodes(&mut self) {
        for node in (0..self.nodes.len()).rev() {
            let mut slots = 0;
            for slot in 0..4 {
                let moved = match self.nodes[node].child(slot) {
                    Child::Empty => false,
                    Child::Node(child) => self.moved.slots(child) != 0,
                    Child::Leaf { start, count } => self.moved.any_position(start, count),
                };
                slots |= u32::from(moved) << slot;
            }
            self.moved.set_slots(node as u32, slots);
        }
    }

    pub fn clear_moved(&mut self) {
        self.moved.clear();
    }
}
