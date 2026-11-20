mod lookahead;
mod pair_set;
mod tree;

use rikka_physics_core::{BitSet, Block, Executor, PerWorker, RadixSorter, Stage};
use rikka_physics_geometry::PairTask;
use rikka_physics_math::Aabb;

use lookahead::Lookahead;
use pair_set::PairSet;
pub use pair_set::{pair_key, pair_of};
use tree::ProxyTree;

use crate::{Filter, PairFilter, worker_reserve};

const QUERY_BLOCK: usize = 32;
const TASKS_PER_WORKER: usize = 8;
const SORTED_MOVES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProxyKind {
    Static,
    Kinematic,
    Dynamic,
}

impl ProxyKind {
    #[inline]
    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug)]
struct ProxyInfo {
    kind: ProxyKind,
    filter: Filter,
    owner: u32,
    contacts: bool,
    alive: bool,
}

const DEAD: ProxyInfo = ProxyInfo {
    kind: ProxyKind::Static,
    filter: Filter {
        category: 0,
        mask: 0,
        group: 0,
    },
    owner: u32::MAX,
    contacts: false,
    alive: false,
};

#[derive(Debug)]
pub struct Broadphase {
    fat: Vec<Aabb>,
    info: Vec<ProxyInfo>,
    trees: [ProxyTree; 3],
    moved: Vec<u32>,
    moved_bits: BitSet,
    moved_by_kind: [Vec<u32>; 3],
    pairs: PairSet,
    tasks: Vec<PairTask>,
    found: PerWorker<Vec<u64>>,
    candidates: Vec<u64>,
    scratch: Vec<u64>,
    sorter: RadixSorter,
}

impl Default for Broadphase {
    fn default() -> Self {
        Self::new()
    }
}

impl Broadphase {
    pub fn new() -> Self {
        Self {
            fat: Vec::new(),
            info: Vec::new(),
            trees: Default::default(),
            moved: Vec::new(),
            moved_bits: BitSet::default(),
            moved_by_kind: Default::default(),
            pairs: PairSet::default(),
            tasks: Vec::new(),
            found: PerWorker::new(1, |_| Vec::new()),
            candidates: Vec::new(),
            scratch: Vec::new(),
            sorter: RadixSorter::new(),
        }
    }

    #[inline]
    pub fn fat_bounds(&self) -> &[Aabb] {
        &self.fat
    }

    #[inline]
    pub fn fat_bounds_mut(&mut self) -> &mut [Aabb] {
        &mut self.fat
    }

    #[inline]
    pub fn fat(&self, proxy: u32) -> Aabb {
        self.fat[proxy as usize]
    }

    pub fn proxy_count(&self) -> usize {
        self.trees.iter().map(ProxyTree::len).sum()
    }

    pub fn create(
        &mut self,
        proxy: u32,
        kind: ProxyKind,
        fat: Aabb,
        filter: Filter,
        owner: u32,
        contacts: bool,
    ) {
        let index = proxy as usize;
        if self.fat.len() <= index {
            self.fat.resize(index + 1, Aabb::EMPTY);
            self.info.resize(index + 1, DEAD);
        }
        debug_assert!(!self.info[index].alive);
        self.fat[index] = fat;
        self.info[index] = ProxyInfo {
            kind,
            filter,
            owner,
            contacts,
            alive: true,
        };
        self.trees[kind.index()].insert(proxy);
        self.mark_moved(proxy);
    }

    pub fn destroy(&mut self, proxy: u32) {
        let info = &mut self.info[proxy as usize];
        debug_assert!(info.alive);
        let kind = info.kind;
        *info = DEAD;
        self.fat[proxy as usize] = Aabb::EMPTY;
        self.trees[kind.index()].remove(proxy);
    }

    pub fn set_kind(&mut self, proxy: u32, kind: ProxyKind) {
        let info = self.info[proxy as usize];
        if info.kind == kind {
            return;
        }
        let fat = self.fat[proxy as usize];
        self.destroy(proxy);
        self.create(proxy, kind, fat, info.filter, info.owner, info.contacts);
    }

    pub fn set_filter(&mut self, proxy: u32, filter: Filter) {
        self.info[proxy as usize].filter = filter;
        self.mark_moved(proxy);
    }

    pub fn move_proxy(&mut self, proxy: u32, fat: Aabb) {
        self.fat[proxy as usize] = fat;
        let kind = self.info[proxy as usize].kind;
        if kind == ProxyKind::Static {
            self.trees[kind.index()].note_change();
        }
        self.mark_moved(proxy);
    }

    pub fn touch(&mut self, proxy: u32) {
        self.mark_moved(proxy);
    }

    pub fn shift_origin(&mut self, offset: rikka_physics_math::Vec3, executor: &dyn Executor) {
        for (bounds, info) in self.fat.iter_mut().zip(&self.info) {
            if info.alive {
                *bounds = bounds.translated(-offset);
            }
        }
        for tree in &mut self.trees {
            tree.rebuild(&self.fat, executor);
        }
    }

    #[inline]
    fn is_moved(&self, proxy: u32) -> bool {
        (proxy as usize) < self.moved_bits.capacity() && self.moved_bits.get(proxy as usize)
    }

    fn mark_moved(&mut self, proxy: u32) {
        self.moved_bits.grow(proxy as usize + 1);
        if !self.moved_bits.get(proxy as usize) {
            self.moved_bits.set(proxy as usize);
            self.moved.push(proxy);
        }
    }

    pub fn remove_pair(&mut self, a: u32, b: u32) {
        self.pairs.remove(pair_key(a, b));
    }

    pub fn has_pair(&self, a: u32, b: u32) -> bool {
        self.pairs.contains(pair_key(a, b))
    }

    pub fn query(&self, bounds: &Aabb, mut visit: impl FnMut(u32) -> bool) -> bool {
        self.trees
            .iter()
            .all(|tree| tree.query(bounds, &self.fat, &mut visit))
    }

    pub fn query_kind(
        &self,
        kind: ProxyKind,
        bounds: &Aabb,
        visit: impl FnMut(u32) -> bool,
    ) -> bool {
        self.trees[kind.index()].query(bounds, &self.fat, visit)
    }

    pub fn closest(
        &self,
        point: rikka_physics_math::Vec3,
        max_distance: f32,
        mut visit: impl FnMut(u32, f32) -> f32,
    ) -> f32 {
        let mut best = max_distance;
        for tree in &self.trees {
            best = tree.bvh().closest(point, best, |proxy, current| {
                if tree.contains_in_tree(proxy) {
                    visit(proxy, current)
                } else {
                    current
                }
            });
            for &proxy in tree.pending() {
                let bounds = self.fat[proxy as usize];
                let gap = (bounds.min - point)
                    .max(point - bounds.max)
                    .max(rikka_physics_math::Vec3::ZERO);
                if gap.length() <= best {
                    best = visit(proxy, best).min(best);
                }
            }
        }
        best
    }

    pub(crate) fn trees(&self) -> &[ProxyTree] {
        &self.trees
    }

    pub fn ray_cast(
        &self,
        origin: rikka_physics_math::Vec3,
        translation: rikka_physics_math::Vec3,
        max_fraction: f32,
        mut visit: impl FnMut(u32, f32) -> f32,
    ) -> f32 {
        let mut best = max_fraction;
        for tree in &self.trees {
            best = tree
                .bvh()
                .ray_cast(origin, translation, best, |proxy, current| {
                    if tree.contains_in_tree(proxy) {
                        visit(proxy, current)
                    } else {
                        current
                    }
                });
            for &proxy in tree.pending() {
                if best <= 0.0 {
                    return best;
                }
                if self.fat[proxy as usize]
                    .ray_fraction(origin, translation, best)
                    .is_some()
                {
                    best = visit(proxy, best);
                }
            }
        }
        best
    }

    #[inline(always)]
    fn stage(&self, lookahead: &mut Lookahead, a: u32, b: u32, ready: impl FnMut(u64)) {
        let key = pair_key(a, b);
        self.pairs.prefetch(key);
        lookahead.push(key, ready);
    }

    #[inline]
    fn is_new_pair(&self, a: u32, b: u32, custom: &dyn PairFilter) -> bool {
        a != b
            && !self.pairs.contains(pair_key(a, b))
            && self.compatible(a, b)
            && custom.should_collide(a.min(b), a.max(b))
    }

    #[inline]
    fn compatible(&self, a: u32, b: u32) -> bool {
        let (x, y) = (&self.info[a as usize], &self.info[b as usize]);
        x.alive
            && y.alive
            && x.owner != y.owner
            && x.contacts
            && y.contacts
            && (x.kind == ProxyKind::Dynamic || y.kind == ProxyKind::Dynamic)
            && x.filter.should_collide(&y.filter)
    }

    pub fn update_pairs(&mut self, executor: &dyn Executor, custom: &dyn PairFilter) -> &[u64] {
        self.candidates.clear();
        if self.moved.is_empty() {
            return &self.candidates;
        }
        if self.moved.len() < SORTED_MOVES {
            self.moved.sort_unstable();
        } else {
            self.moved.clear();
            self.moved
                .extend(self.moved_bits.iter().map(|proxy| proxy as u32));
        }
        if self.found.len() != executor.workers() {
            self.found = PerWorker::new(executor.workers(), |_| Vec::new());
        }
        for list in &mut self.moved_by_kind {
            list.clear();
        }
        for &proxy in &self.moved {
            let info = self.info[proxy as usize];
            if info.alive {
                self.moved_by_kind[info.kind.index()].push(proxy);
            }
        }

        for kind in [ProxyKind::Static, ProxyKind::Kinematic, ProxyKind::Dynamic] {
            let index = kind.index();
            if self.trees[index].needs_rebuild() {
                self.trees[index].rebuild(&self.fat, executor);
            } else {
                self.trees[index].refit_moved(&self.moved_by_kind[index], &self.fat, executor);
            }
        }
        let dynamic = ProxyKind::Dynamic.index();
        let full = 2 * self.moved_by_kind[dynamic].len() > self.trees[dynamic].len();
        if !full {
            self.trees[dynamic].mark_moved(&self.moved_by_kind[dynamic]);
        }

        self.find_pairs(executor, custom, full);
        self.trees[dynamic].clear_moved();
        for &proxy in &self.moved {
            self.moved_bits.clear(proxy as usize);
        }
        self.moved.clear();

        for found in self.found.iter_mut() {
            self.candidates.append(found);
        }
        let workers = self.found.len().max(1);
        let share = worker_reserve(self.candidates.len(), workers);
        for found in self.found.iter_mut() {
            found.reserve(share);
        }
        if self.scratch.len() < self.candidates.len() {
            self.scratch.resize(self.candidates.len(), 0);
        }
        self.sorter
            .sort(&mut self.candidates, &mut self.scratch[..], |key: &u64| {
                *key
            });
        self.candidates.dedup();
        debug_assert!(self.candidates.iter().all(|&key| !self.pairs.contains(key)));
        self.pairs.insert_all(&self.candidates);
        &self.candidates
    }

    fn find_pairs(&mut self, executor: &dyn Executor, custom: &dyn PairFilter, full: bool) {
        let bvh = self.trees[ProxyKind::Dynamic.index()].bvh();
        let target = executor.workers() * TASKS_PER_WORKER;
        if full {
            bvh.self_pair_tasks(target, &mut self.tasks);
        } else {
            bvh.moved_pair_tasks(target, &mut self.tasks);
        }
        let [fixed, kinematic, moving] = &self.moved_by_kind;
        let firsts = [
            0,
            self.tasks.len(),
            self.tasks.len() + fixed.len(),
            self.tasks.len() + fixed.len() + kinematic.len(),
            self.tasks.len() + fixed.len() + kinematic.len() + moving.len().div_ceil(QUERY_BLOCK),
        ];
        let this = &*self;
        let task = |block: Block| {
            let index = block.index as usize;
            let slot = firsts.partition_point(|&first| first <= index) - 1;
            this.find_block(slot, index - firsts[slot], block.worker, custom, full);
        };
        executor.run(
            &[Stage::new("broadphase moved pairs", firsts[4] as u32)],
            &task,
        );
    }

    fn find_block(
        &self,
        slot: usize,
        index: usize,
        worker: u32,
        custom: &dyn PairFilter,
        full: bool,
    ) {
        let dynamic = &self.trees[ProxyKind::Dynamic.index()];
        let mut found = self.found.local(worker);
        let mut lookahead = Lookahead::new();
        let mut ready = |key: u64| {
            let (a, b) = pair_of(key);
            if self.is_new_pair(a, b, custom) {
                found.push(key);
            }
        };
        match slot {
            0 => {
                dynamic.bvh().run_self_pair_task(self.tasks[index], |a, b| {
                    if dynamic.contains_in_tree(a)
                        && dynamic.contains_in_tree(b)
                        && (!full || self.is_moved(a) || self.is_moved(b))
                    {
                        self.stage(&mut lookahead, a, b, &mut ready);
                    }
                    true
                });
            }
            1 | 2 => {
                let proxy = self.moved_by_kind[slot - 1][index];
                let bounds = self.fat[proxy as usize];
                dynamic.query(&bounds, &self.fat, |other| {
                    if !self.is_moved(other) {
                        self.stage(&mut lookahead, proxy, other, &mut ready);
                    }
                    true
                });
            }
            _ => {
                let moved = &self.moved_by_kind[ProxyKind::Dynamic.index()];
                let start = index * QUERY_BLOCK;
                for &proxy in &moved[start..(start + QUERY_BLOCK).min(moved.len())] {
                    let bounds = self.fat[proxy as usize];
                    for kind in [ProxyKind::Static, ProxyKind::Kinematic] {
                        self.trees[kind.index()].query(&bounds, &self.fat, |other| {
                            self.stage(&mut lookahead, proxy, other, &mut ready);
                            true
                        });
                    }
                    if dynamic.contains_in_tree(proxy) {
                        dynamic.query_pending(&bounds, &self.fat, |other| {
                            if !self.is_moved(other) {
                                self.stage(&mut lookahead, proxy, other, &mut ready);
                            }
                            true
                        });
                    } else {
                        dynamic.query(&bounds, &self.fat, |other| {
                            if !(self.is_moved(other)
                                && !dynamic.contains_in_tree(other)
                                && other < proxy)
                            {
                                self.stage(&mut lookahead, proxy, other, &mut ready);
                            }
                            true
                        });
                    }
                }
            }
        }
        lookahead.drain(&mut ready);
    }
}
