use rikka_physics_core::{BitSet, Executor};
use rikka_physics_geometry::Bvh4;
use rikka_physics_math::Aabb;

const NOT_MEMBER: u32 = u32::MAX;
const REBUILD_PENDING: usize = 64;
const REBUILD_FRACTION: f32 = 0.1;
const SUBTREE_LIMIT: usize = 32;
const SMALL_TREE: usize = 512;
const PARALLEL_TREE: usize = 4096;
const QUALITY_CHECK: u32 = 32;
const DEGRADED_COST: f32 = 1.5;

#[derive(Debug, Default)]
pub(crate) struct ProxyTree {
    bvh: Bvh4,
    members: Vec<u32>,
    slots: Vec<u32>,
    pending: Vec<u32>,
    in_tree: BitSet,
    changed: usize,
    built: usize,
    built_cost: f32,
    refits: u32,
    degraded: bool,
    marks: BitSet,
    roots: Vec<u32>,
}

impl ProxyTree {
    pub(crate) fn len(&self) -> usize {
        self.members.len()
    }

    pub(crate) fn pending(&self) -> &[u32] {
        &self.pending
    }

    pub(crate) fn bvh(&self) -> &Bvh4 {
        &self.bvh
    }

    pub(crate) fn contains_in_tree(&self, proxy: u32) -> bool {
        (proxy as usize) < self.in_tree.capacity() && self.in_tree.get(proxy as usize)
    }

    pub(crate) fn insert(&mut self, proxy: u32) {
        let index = proxy as usize;
        if self.slots.len() <= index {
            self.slots.resize(index + 1, NOT_MEMBER);
        }
        self.slots[index] = self.members.len() as u32;
        self.members.push(proxy);
        self.pending.push(proxy);
        self.changed += 1;
    }

    pub(crate) fn remove(&mut self, proxy: u32) {
        let slot = self.slots[proxy as usize];
        self.slots[proxy as usize] = NOT_MEMBER;
        self.members.swap_remove(slot as usize);
        if let Some(&moved) = self.members.get(slot as usize) {
            self.slots[moved as usize] = slot;
        }
        if self.contains_in_tree(proxy) {
            self.in_tree.clear(proxy as usize);
        }
        if let Some(position) = self.pending.iter().position(|&p| p == proxy) {
            self.pending.swap_remove(position);
        }
        self.changed += 1;
    }

    pub(crate) fn note_change(&mut self) {
        self.changed += 1;
    }

    pub(crate) fn needs_rebuild(&self) -> bool {
        let fraction = self.changed as f32 / self.built.max(1) as f32;
        self.degraded
            || self.pending.len() > REBUILD_PENDING
            || (self.changed > 0 && fraction > REBUILD_FRACTION)
    }

    pub(crate) fn rebuild(&mut self, bounds: &[Aabb], executor: &dyn Executor) {
        let builder = Bvh4::builder();
        let members = self.members.iter().copied();
        if self.members.len() > PARALLEL_TREE {
            builder
                .executor(executor)
                .build_into(&mut self.bvh, bounds, members);
        } else {
            builder.build_into(&mut self.bvh, bounds, members);
        }
        for &member in &self.members {
            self.in_tree.set_growing(member as usize);
        }
        self.pending.clear();
        self.changed = 0;
        self.built = self.members.len();
        self.built_cost = self.bvh.sah_cost();
        self.refits = 0;
        self.degraded = false;
    }

    pub(crate) fn refit_moved(&mut self, moved: &[u32], bounds: &[Aabb], executor: &dyn Executor) {
        if self.bvh.is_empty() || moved.is_empty() {
            return;
        }
        self.refits += 1;
        if self.refits.is_multiple_of(QUALITY_CHECK)
            && self.bvh.sah_cost() > DEGRADED_COST * self.built_cost
        {
            self.degraded = true;
        }
        if moved.len() > SUBTREE_LIMIT
            || 4 * moved.len() > self.members.len()
            || self.members.len() <= SMALL_TREE
        {
            if self.members.len() > PARALLEL_TREE {
                self.bvh.refit_on(bounds, executor);
            } else {
                self.bvh.refit(bounds);
            }
            return;
        }
        self.marks.clear_all();
        self.marks.grow(self.bvh.nodes().len());
        self.roots.clear();
        for &proxy in moved {
            if !self.contains_in_tree(proxy) {
                continue;
            }
            let Some(location) = self.bvh.location(proxy) else {
                continue;
            };
            let root = self
                .bvh
                .parent(location.node)
                .map_or(location.node, |parent| parent.node);
            if !self.marks.get(root as usize) {
                self.marks.set(root as usize);
                self.roots.push(root);
            }
        }
        let bvh = &self.bvh;
        let marks = &self.marks;
        self.roots.retain(|&root| {
            let mut ancestor = bvh.parent(root);
            while let Some(location) = ancestor {
                if marks.get(location.node as usize) {
                    return false;
                }
                ancestor = bvh.parent(location.node);
            }
            true
        });
        self.roots.sort_unstable();
        for &root in &self.roots {
            self.bvh.rebuild_subtree(root, bounds);
        }
    }

    pub(crate) fn query(
        &self,
        bounds: &Aabb,
        fat: &[Aabb],
        mut visit: impl FnMut(u32) -> bool,
    ) -> bool {
        if !self.bvh.query(*bounds, |proxy| {
            !self.contains_in_tree(proxy) || !fat[proxy as usize].overlaps(bounds) || visit(proxy)
        }) {
            return false;
        }
        self.query_pending(bounds, fat, visit)
    }

    pub(crate) fn query_pending(
        &self,
        bounds: &Aabb,
        fat: &[Aabb],
        mut visit: impl FnMut(u32) -> bool,
    ) -> bool {
        self.pending
            .iter()
            .all(|&proxy| !fat[proxy as usize].overlaps(bounds) || visit(proxy))
    }

    pub(crate) fn mark_moved(&mut self, moved: &[u32]) {
        if !self.bvh.is_empty() && !moved.is_empty() {
            self.bvh.mark_moved(moved.iter().copied().filter(|&proxy| {
                (proxy as usize) < self.in_tree.capacity() && self.in_tree.get(proxy as usize)
            }));
        }
    }

    pub(crate) fn clear_moved(&mut self) {
        self.bvh.clear_moved();
    }
}
