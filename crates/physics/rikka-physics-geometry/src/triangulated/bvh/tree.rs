use rikka_sync::Mutex;

use rikka_physics_core::{Executor, ExecutorExt};
use rikka_physics_math::{Aabb, Vec3};

use super::bounds4::{Bounds4, QueryBounds4};
use super::build::{Builder, Task};
use super::item_map::ItemMap;
use super::items::ItemBounds;
use super::moved::MovedMarks;
use super::node::{Child, EMPTY_CHILD, MAX_LEAF_SIZE, Node4};
use super::pairs::{PairTask, Pairs};
use super::quantized::{QuantizedBvh4, quantize_node};
use super::stack::Stack;
use super::traverse::{self, Nodes};

const PARALLEL_GRAIN: u32 = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    pub node: u32,
    pub slot: u32,
}

#[must_use]
#[derive(Clone, Copy)]
pub struct Bvh4Builder<'a> {
    leaf_size: u32,
    bins: u32,
    executor: Option<&'a dyn Executor>,
}

impl std::fmt::Debug for Bvh4Builder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bvh4Builder")
            .field("leaf_size", &self.leaf_size)
            .field("bins", &self.bins)
            .field("parallel", &self.executor.is_some())
            .finish()
    }
}

impl<'a> Bvh4Builder<'a> {
    pub fn leaf_size(mut self, leaf_size: u32) -> Self {
        self.leaf_size = leaf_size.clamp(1, MAX_LEAF_SIZE);
        self
    }

    pub fn bins(mut self, bins: u32) -> Self {
        self.bins = bins;
        self
    }

    pub fn executor<'b>(self, executor: &'b dyn Executor) -> Bvh4Builder<'b> {
        Bvh4Builder {
            leaf_size: self.leaf_size,
            bins: self.bins,
            executor: Some(executor),
        }
    }

    pub fn build_into(
        self,
        tree: &mut Bvh4,
        bounds: &[Aabb],
        items: impl IntoIterator<Item = u32>,
    ) {
        tree.moved.clear();
        tree.nodes.clear();
        tree.parents.clear();
        tree.free.clear();
        tree.items.clear();
        tree.items.extend(items);
        tree.positions.reset(tree.items.len());
        tree.leaves.clear();
        tree.leaves.resize(tree.items.len(), EMPTY_CHILD);
        tree.leaf_size = self.leaf_size;
        tree.bins = self.bins;
        if !tree.items.is_empty() {
            let builder = Builder::new(bounds, self.leaf_size, self.bins);
            tree.nodes.push(Node4::EMPTY);
            let mut deferred = std::mem::take(&mut tree.deferred);
            builder.build(
                &mut tree.items,
                0,
                0,
                &mut tree.nodes,
                Some(PARALLEL_GRAIN),
                &mut deferred,
            );
            deferred.sort_unstable_by_key(|task| task.start);
            match self.executor {
                Some(executor) if executor.workers() > 1 && deferred.len() > 1 => {
                    build_parallel(&builder, tree, &deferred, executor);
                }
                _ => build_serial(&builder, tree, &deferred),
            }
            deferred.clear();
            tree.deferred = deferred;
            tree.parents.resize(tree.nodes.len(), EMPTY_CHILD);
            tree.link(0, EMPTY_CHILD);
        }
        tree.item_bounds.assign(&tree.items, bounds);
    }

    pub fn build(self, bounds: &[Aabb], items: impl IntoIterator<Item = u32>) -> Bvh4 {
        let mut tree = Bvh4::default();
        self.build_into(&mut tree, bounds, items);
        tree
    }
}

fn build_serial(builder: &Builder<'_>, tree: &mut Bvh4, deferred: &[Task]) {
    for task in deferred {
        let range = task.start as usize..(task.start + task.count) as usize;
        builder.build(
            &mut tree.items[range],
            task.start,
            task.node,
            &mut tree.nodes,
            None,
            &mut Vec::new(),
        );
    }
}

fn build_parallel(
    builder: &Builder<'_>,
    tree: &mut Bvh4,
    deferred: &[Task],
    executor: &dyn Executor,
) {
    let subtrees = build_deferred(builder, &mut tree.items, deferred, executor);
    for (task, local) in deferred.iter().zip(subtrees) {
        let offset = tree.nodes.len() as u32;
        let remap = |child: u32| {
            if child == 0 {
                task.node
            } else {
                offset + child - 1
            }
        };
        for (index, mut node) in local.into_iter().enumerate() {
            for slot in 0..4 {
                if let Child::Node(child) = node.child(slot) {
                    node.set_child(slot, Child::Node(remap(child)));
                }
            }
            if index == 0 {
                tree.nodes[task.node as usize] = node;
            } else {
                tree.nodes.push(node);
            }
        }
    }
}

struct Subtree<'a> {
    items: &'a mut [u32],
    start: u32,
    nodes: Vec<Node4>,
}

fn build_deferred(
    builder: &Builder<'_>,
    items: &mut [u32],
    deferred: &[Task],
    executor: &dyn Executor,
) -> Vec<Vec<Node4>> {
    let mut rest = items;
    let mut consumed = 0u32;
    let mut work = Vec::with_capacity(deferred.len());
    for task in deferred {
        let (_, tail) = rest.split_at_mut((task.start - consumed) as usize);
        let (slice, tail) = tail.split_at_mut(task.count as usize);
        work.push(Mutex::new(Subtree {
            items: slice,
            start: task.start,
            nodes: vec![Node4::EMPTY],
        }));
        rest = tail;
        consumed = task.start + task.count;
    }
    executor.for_each(deferred.len() as u32, |block| {
        let mut guard = work[block.index as usize].lock();
        let subtree = &mut *guard;
        builder.build(
            subtree.items,
            subtree.start,
            0,
            &mut subtree.nodes,
            None,
            &mut Vec::new(),
        );
    });
    work.into_iter()
        .map(|cell| cell.into_inner().nodes)
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bvh4 {
    pub(super) nodes: Vec<Node4>,
    pub(super) parents: Vec<u32>,
    pub(super) items: Vec<u32>,
    pub(super) item_bounds: ItemBounds,
    pub(super) moved: MovedMarks,
    pub(super) positions: ItemMap,
    pub(super) leaves: Vec<u32>,
    pub(super) free: Vec<u32>,
    deferred: Vec<Task>,
    pub(super) leaf_size: u32,
    pub(super) bins: u32,
}

impl Bvh4 {
    pub fn builder() -> Bvh4Builder<'static> {
        Bvh4Builder {
            leaf_size: 4,
            bins: 16,
            executor: None,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    #[inline]
    pub fn root(&self) -> Option<u32> {
        (!self.nodes.is_empty()).then_some(0)
    }

    #[inline]
    pub fn nodes(&self) -> &[Node4] {
        &self.nodes
    }

    #[inline]
    pub fn items(&self) -> &[u32] {
        &self.items
    }

    #[inline]
    pub fn node(&self, index: u32) -> &Node4 {
        &self.nodes[index as usize]
    }

    pub fn parent(&self, node: u32) -> Option<Location> {
        decode_location(self.parents[node as usize])
    }

    pub fn location(&self, item: u32) -> Option<Location> {
        self.positions
            .get(item)
            .and_then(|position| decode_location(self.leaves[position as usize]))
    }

    pub fn bounds(&self) -> Aabb {
        self.nodes.first().map_or(Aabb::EMPTY, Node4::union)
    }

    pub fn height(&self) -> u32 {
        let Some(root) = self.root() else {
            return 0;
        };
        let mut height = 0;
        let mut stack = Stack::new();
        stack.push((root, 1u32));
        while let Some((node, depth)) = stack.pop() {
            height = height.max(depth);
            for slot in 0..4 {
                if let Child::Node(child) = self.nodes[node as usize].child(slot) {
                    stack.push((child, depth + 1));
                }
            }
        }
        height
    }

    pub fn sah_cost(&self) -> f32 {
        let Some(root) = self.root() else {
            return 0.0;
        };
        let root_area = self.bounds().surface_area().max(f32::MIN_POSITIVE);
        let mut cost = 0.0;
        let mut stack = Stack::new();
        stack.push(root);
        while let Some(node) = stack.pop() {
            let node = &self.nodes[node as usize];
            for slot in 0..4 {
                let area = node.bounds(slot).surface_area();
                match node.child(slot) {
                    Child::Empty => {}
                    Child::Node(child) => {
                        cost += area;
                        stack.push(child);
                    }
                    Child::Leaf { count, .. } => cost += area * count as f32,
                }
            }
        }
        cost / root_area
    }

    pub fn query(&self, bounds: Aabb, mut visit: impl FnMut(u32) -> bool) -> bool {
        let query = QueryBounds4::new(&bounds);
        traverse::query_leaves_splatted(self, &query, |start, count| {
            self.item_bounds
                .overlapping(start, count, &query)
                .all(|position| visit(self.items[position as usize]))
        })
    }

    pub fn ray_cast(
        &self,
        origin: Vec3,
        translation: Vec3,
        max_fraction: f32,
        mut visit: impl FnMut(u32, f32) -> f32,
    ) -> f32 {
        traverse::sweep(
            self,
            origin,
            translation,
            Vec3::ZERO,
            max_fraction,
            |slot, best| visit(self.items[slot as usize], best),
        )
    }

    pub fn box_cast(
        &self,
        bounds: Aabb,
        translation: Vec3,
        max_fraction: f32,
        mut visit: impl FnMut(u32, f32) -> f32,
    ) -> f32 {
        traverse::sweep(
            self,
            bounds.center(),
            translation,
            bounds.extent(),
            max_fraction,
            |slot, best| visit(self.items[slot as usize], best),
        )
    }

    pub fn closest(
        &self,
        point: Vec3,
        max_distance: f32,
        mut visit: impl FnMut(u32, f32) -> f32,
    ) -> f32 {
        let best = traverse::closest(self, point, max_distance * max_distance, |slot, best| {
            let distance = visit(self.items[slot as usize], best.sqrt());
            distance * distance
        });
        best.max(0.0).sqrt()
    }

    pub fn self_pairs(&self, mut visit: impl FnMut(u32, u32) -> bool) -> bool {
        let pairs = Pairs::self_pairs(self, false);
        pairs
            .self_root()
            .is_none_or(|root| pairs.run(root, &mut visit))
    }

    pub fn tree_pairs(&self, other: &Bvh4, mut visit: impl FnMut(u32, u32) -> bool) -> bool {
        let pairs = Pairs::cross_pairs(self, other);
        pairs
            .cross_root()
            .is_none_or(|root| pairs.run(root, &mut visit))
    }

    pub fn self_pair_tasks(&self, target: usize, tasks: &mut Vec<PairTask>) {
        Pairs::self_pairs(self, false).tasks(target, tasks);
    }

    pub fn moved_pair_tasks(&self, target: usize, tasks: &mut Vec<PairTask>) {
        Pairs::self_pairs(self, true).tasks(target, tasks);
    }

    pub fn run_self_pair_task(
        &self,
        task: PairTask,
        mut visit: impl FnMut(u32, u32) -> bool,
    ) -> bool {
        Pairs::self_pairs(self, task.moved_only).run(task, &mut visit)
    }

    pub fn quantize(&self) -> QuantizedBvh4 {
        let Some(root) = self.root() else {
            return QuantizedBvh4::default();
        };
        let bounds = self.bounds();
        let mut nodes = vec![Default::default()];
        let mut stack = vec![(root, 0u32, bounds)];
        while let Some((source, target, frame)) = stack.pop() {
            let node = &self.nodes[source as usize];
            let mut children = [(Child::Empty, Aabb::EMPTY); 4];
            let mut pending = [None; 4];
            for (slot, entry) in children.iter_mut().enumerate() {
                *entry = match node.child(slot) {
                    Child::Node(child) => {
                        let index = nodes.len() as u32;
                        nodes.push(Default::default());
                        pending[slot] = Some((child, index));
                        (Child::Node(index), node.bounds(slot))
                    }
                    other => (other, node.bounds(slot)),
                };
            }
            let (quantized, decoded) = quantize_node(&frame, children);
            nodes[target as usize] = quantized;
            for (slot, pending) in pending.into_iter().enumerate().rev() {
                if let Some((child, index)) = pending {
                    stack.push((child, index, decoded.lane(slot)));
                }
            }
        }
        QuantizedBvh4 { nodes, bounds }
    }
}

#[inline]
pub(super) fn decode_location(location: u32) -> Option<Location> {
    (location != EMPTY_CHILD).then_some(Location {
        node: location >> 2,
        slot: location & 3,
    })
}

impl Nodes for Bvh4 {
    type Frame = ();

    #[inline]
    fn root(&self) -> Option<(u32, ())> {
        (!self.nodes.is_empty()).then_some((0, ()))
    }

    #[inline]
    fn load(&self, node: u32, _: &()) -> Bounds4 {
        let node = &self.nodes[node as usize];
        Bounds4::from_f32(
            [&node.min_x, &node.min_y, &node.min_z],
            [&node.max_x, &node.max_y, &node.max_z],
        )
    }

    #[inline]
    fn child(&self, node: u32, slot: usize) -> Child {
        self.nodes[node as usize].child(slot)
    }

    #[inline]
    fn occupied(&self, node: u32) -> u32 {
        let children = &self.nodes[node as usize].children;
        (0..4).fold(0, |mask, slot| {
            mask | (u32::from(children[slot] != EMPTY_CHILD) << slot)
        })
    }

    #[inline]
    fn child_frame(_: &Bounds4, _: usize) {}
}
