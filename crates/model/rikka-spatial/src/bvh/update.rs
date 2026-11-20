use smallvec::SmallVec;

use super::{Bvh, BvhUpdate, Node, Primitive, build};
use crate::aabb::Aabb;

const REBUILD_COST_RATIO: f32 = 1.5;
const ROTATION_GAIN: f32 = 1e-5;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Restructure {
    Keep,
    Rotate,
}

#[derive(Debug, Clone)]
pub(crate) struct Links {
    slot_of: Vec<u32>,
    leaf_of: Vec<u32>,
    parent: Vec<u32>,
}

impl Links {
    fn new(nodes: &[Node], primitives: &[Primitive]) -> Links {
        let mut links = Links {
            slot_of: vec![0; primitives.len()],
            leaf_of: vec![0; primitives.len()],
            parent: vec![u32::MAX; nodes.len()],
        };
        for (slot, primitive) in primitives.iter().enumerate() {
            links.slot_of[primitive.id as usize] = slot as u32;
        }
        for (index, node) in nodes.iter().enumerate() {
            match node.is_leaf() {
                true => {
                    for slot in node.start..node.start + node.count {
                        links.leaf_of[slot as usize] = index as u32;
                    }
                }
                false => {
                    links.parent[node.start as usize] = index as u32;
                    links.parent[node.start as usize + 1] = index as u32;
                }
            }
        }
        links
    }
}

impl Bvh {
    pub fn refit(&mut self, bounds: &[Aabb]) {
        self.store_bounds(bounds);
        self.refit_nodes(Restructure::Keep);
    }

    pub fn update(&mut self, bounds: &[Aabb]) -> BvhUpdate {
        self.store_bounds(bounds);
        self.refit_nodes(Restructure::Rotate);
        self.links = None;
        if self.cost() <= self.built_cost * REBUILD_COST_RATIO {
            return BvhUpdate::Refitted;
        }
        let (nodes, primitives) = build::rebuild_primitives(&self.primitives, self.settings);
        self.nodes = nodes;
        self.primitives = primitives;
        self.built_cost = self.cost();
        BvhUpdate::Rebuilt
    }

    pub fn refit_primitives(&mut self, changed: impl IntoIterator<Item = (u32, Aabb)>) {
        if self.nodes.is_empty() {
            return;
        }
        let links = self
            .links
            .get_or_insert_with(|| Box::new(Links::new(&self.nodes, &self.primitives)));
        let mut leaves: Vec<u32> = Vec::new();
        for (id, bounds) in changed {
            let slot = links.slot_of[id as usize] as usize;
            let updated = build::sanitize(&bounds);
            let primitive = &mut self.primitives[slot];
            primitive.min = updated.min;
            primitive.max = updated.max;
            leaves.push(links.leaf_of[slot]);
        }
        leaves.sort_unstable();
        leaves.dedup();
        let links = self.links.take().expect("links were built above");
        for leaf in leaves {
            let node = self.nodes[leaf as usize];
            let bounds = self
                .leaf(&node)
                .iter()
                .fold(Aabb::EMPTY, |bounds, primitive| {
                    bounds.union(&primitive.bounds())
                });
            self.nodes[leaf as usize].set_bounds(&bounds);
            let mut child = leaf;
            while links.parent[child as usize] != u32::MAX {
                let parent = links.parent[child as usize];
                let first = self.nodes[parent as usize].start as usize;
                let bounds = self.pair_bounds(first);
                let old = self.nodes[parent as usize].bounds();
                if old.min == bounds.min && old.max == bounds.max {
                    break;
                }
                self.nodes[parent as usize].set_bounds(&bounds);
                child = parent;
            }
        }
        self.links = Some(links);
    }

    fn store_bounds(&mut self, bounds: &[Aabb]) {
        assert_eq!(
            bounds.len(),
            self.primitives.len(),
            "one bound per bvh primitive"
        );
        for primitive in &mut self.primitives {
            let updated = build::sanitize(&bounds[primitive.id as usize]);
            primitive.min = updated.min;
            primitive.max = updated.max;
        }
    }

    fn refit_nodes(&mut self, restructure: Restructure) {
        if self.nodes.is_empty() {
            return;
        }
        let mut stack: SmallVec<[(u32, bool); 64]> = SmallVec::new();
        stack.push((0, false));
        while let Some((index, expanded)) = stack.pop() {
            let node = self.nodes[index as usize];
            if node.is_leaf() {
                let bounds = self
                    .leaf(&node)
                    .iter()
                    .fold(Aabb::EMPTY, |bounds, primitive| {
                        bounds.union(&primitive.bounds())
                    });
                self.nodes[index as usize].set_bounds(&bounds);
                continue;
            }
            if !expanded {
                stack.push((index, true));
                stack.push((node.start + 1, false));
                stack.push((node.start, false));
                continue;
            }
            if restructure == Restructure::Rotate {
                self.rotate(node.start as usize);
            }
            let bounds = self.pair_bounds(node.start as usize);
            self.nodes[index as usize].set_bounds(&bounds);
        }
    }

    fn pair_bounds(&self, first: usize) -> Aabb {
        self.nodes[first]
            .bounds()
            .union(&self.nodes[first + 1].bounds())
    }

    fn rotate(&mut self, first: usize) {
        let mut best: Option<(f32, usize, usize)> = None;
        for (moved, parent) in [(first, first + 1), (first + 1, first)] {
            let parent_node = self.nodes[parent];
            if parent_node.is_leaf() {
                continue;
            }
            let area = parent_node.bounds().surface_area();
            let grandchild = parent_node.start as usize;
            for (swapped, kept) in [(grandchild, grandchild + 1), (grandchild + 1, grandchild)] {
                let rotated = self.nodes[moved]
                    .bounds()
                    .union(&self.nodes[kept].bounds())
                    .surface_area();
                let gain = area - rotated;
                if gain > ROTATION_GAIN * area
                    && best.is_none_or(|(best_gain, _, _)| gain > best_gain)
                {
                    best = Some((gain, moved, swapped));
                }
            }
        }
        let Some((_, moved, swapped)) = best else {
            return;
        };
        self.nodes.swap(moved, swapped);
        let parent = if moved == first { first + 1 } else { first };
        let grandchildren = self.nodes[parent].start as usize;
        let bounds = self.pair_bounds(grandchildren);
        self.nodes[parent].set_bounds(&bounds);
    }
}
