mod geometry;
mod islands;
mod labels;

use rikka_types::{Point, Rect};

use crate::runtime::instance::Instance;
use crate::runtime::layout::{Layouts, Node};
use crate::runtime::scene_sync::PrimSlot;
use crate::view::Behavior;

pub(crate) use geometry::{Clipping, Reach, Seen};
pub(crate) use islands::{Bound, DissolvedIsland, Islands};
pub(crate) use labels::Labels;

pub(crate) const CULLED: u16 = 1;
pub(crate) const ENTERED: u16 = 1 << 1;
pub(crate) const PAINT: u16 = 1 << 2;
pub(crate) const BELOW: u16 = 1 << 3;
pub(crate) const VISIT: u16 = 1 << 4;
pub(crate) const ZOOMED: u16 = 1 << 5;
pub(crate) const TRACKS: u16 = 1 << 6;
pub(crate) const TRACKS_BELOW: u16 = 1 << 7;
pub(crate) const PENDING: u16 = PAINT | BELOW | VISIT;

#[derive(Default)]
pub(crate) struct Table {
    cold: Vec<Option<Box<Instance>>>,
    generation: Vec<u32>,
    behavior: Vec<Behavior>,
    placed: Vec<Rect>,
    inner: Vec<Point>,
    extent: Vec<Rect>,
    clipping: Vec<Clipping>,
    reach: Vec<Reach>,
    flags: Vec<u16>,
    entered: Vec<Node>,
    pub(crate) labels: Labels,
    pub(crate) islands: Islands,
}

impl Table {
    pub(crate) fn id_for(&mut self, node: Node) -> u64 {
        let at = node.index();
        if self.generation.len() <= at {
            let needed_len = at + 1;
            self.cold.resize_with(needed_len, || None);
            self.generation.resize(needed_len, 0);
            self.behavior.resize(needed_len, Behavior::NONE);
            self.placed.resize(needed_len, Rect::ZERO);
            self.inner.resize(needed_len, Point::ZERO);
            self.extent.resize(needed_len, Rect::ZERO);
            self.clipping.resize(needed_len, Clipping::default());
            self.reach.resize(needed_len, Reach::OPEN);
            self.flags.resize(needed_len, 0);
        }
        self.generation[at] += 1;
        self.behavior[at] = Behavior::NONE;
        self.placed[at] = Rect::ZERO;
        self.inner[at] = Point::ZERO;
        self.extent[at] = Rect::ZERO;
        self.clipping[at] = Clipping::default();
        self.reach[at] = Reach::OPEN;
        self.flags[at] = PAINT;
        self.labels.single(node);
        self.islands.insert(node);
        (u64::from(self.generation[at]) << 32) | u64::from(node.slot())
    }

    pub(crate) fn put(&mut self, node: Node, instance: Box<Instance>) {
        self.behavior[node.index()] = instance.behavior;
        self.cold[node.index()] = Some(instance);
    }

    pub(crate) fn take(&mut self, node: Node) -> Box<Instance> {
        self.cold[node.index()].take().expect("node not in table")
    }

    pub(crate) fn prefetch(&self, node: Node) {
        self.prefetch_head(node, size_of::<Instance>());
    }

    pub(crate) fn prefetch_head(&self, node: Node, bytes: usize) {
        if let Some(Some(instance)) = self.cold.get(node.index()) {
            rikka_simd::prefetch(&raw const **instance, bytes);
        }
    }

    pub(crate) fn prefetch_prims(&self, node: Node) {
        let prims = self.prims(node);
        rikka_simd::prefetch(prims.as_ptr(), size_of_val(prims));
    }

    pub(crate) fn prims(&self, node: Node) -> &[PrimSlot] {
        match self.cold.get(node.index()) {
            Some(Some(instance)) => &instance.prims,
            _ => &[],
        }
    }

    pub(crate) fn find(&self, id: u64) -> Option<Node> {
        let slot = id as u32;
        let generation = (id >> 32) as u32;
        let at = slot as usize;
        (self.generation.get(at) == Some(&generation) && self.cold.get(at)?.is_some())
            .then(|| Node::from_slot(slot))
    }

    pub(crate) fn get(&self, node: Node) -> &Instance {
        self.cold[node.index()]
            .as_deref()
            .expect("node not in table")
    }

    pub(crate) fn get_mut(&mut self, node: Node) -> &mut Instance {
        self.cold[node.index()]
            .as_deref_mut()
            .expect("node not in table")
    }

    pub(crate) fn relabel(&mut self, layouts: &Layouts, scene: &mut rikka_renderer::Scene) {
        for &node in self.labels.relabeled() {
            let Some(instance) = self
                .cold
                .get_mut(node.index())
                .and_then(Option::as_deref_mut)
            else {
                continue;
            };
            crate::runtime::scene_sync::relabel_prims(
                scene,
                &mut instance.prims,
                &self.labels,
                node,
            );
            if self.clipping(node).raises {
                mark_path(&mut self.flags, layouts, node, VISIT);
            }
        }
        self.labels.settle_relabeled();
    }

    pub(crate) fn remove(&mut self, node: Node) -> Option<Box<Instance>> {
        if self.has(node, ENTERED) {
            self.entered.retain(|entered_node| *entered_node != node);
        }
        self.flags[node.index()] = 0;
        self.cold[node.index()].take()
    }

    pub(crate) fn entered(&self) -> &[Node] {
        &self.entered
    }

    pub(crate) fn enter(&mut self, node: Node) -> bool {
        if self.has(node, ENTERED) {
            return false;
        }
        self.raise(node, ENTERED);
        self.entered.push(node);
        true
    }

    pub(crate) fn leave(&mut self, node: Node) -> bool {
        if !self.has(node, ENTERED) {
            return false;
        }
        self.lower(node, ENTERED);
        self.entered.retain(|entered_node| *entered_node != node);
        true
    }

    pub(crate) fn holds(&self, node: Node) -> bool {
        self.cold.get(node.index()).is_some_and(Option::is_some)
    }

    pub(crate) fn behavior(&self, node: Node) -> Behavior {
        self.behavior[node.index()]
    }

    pub(crate) fn refresh_behavior(&mut self, node: Node) {
        let instance = self.get_mut(node);
        instance.refresh_behavior();
        let behavior = instance.behavior;
        self.behavior[node.index()] = behavior;
    }

    pub(crate) fn placed(&self, node: Node) -> Rect {
        self.placed[node.index()]
    }

    pub(crate) fn set_placed(&mut self, node: Node, placed: Rect) {
        self.placed[node.index()] = placed;
    }

    pub(crate) fn inner(&self, node: Node) -> Point {
        self.inner[node.index()]
    }

    pub(crate) fn set_inner(&mut self, node: Node, inner: Point) {
        self.inner[node.index()] = inner;
    }

    pub(crate) fn extent(&self, node: Node) -> Rect {
        self.extent[node.index()]
    }

    pub(crate) fn set_extent(&mut self, node: Node, extent: Rect) {
        self.extent[node.index()] = extent;
    }

    pub(crate) fn clipping(&self, node: Node) -> Clipping {
        self.clipping[node.index()]
    }

    pub(crate) fn set_clipping(&mut self, node: Node, clipping: Clipping) {
        self.clipping[node.index()] = clipping;
    }

    pub(crate) fn reach(&self, node: Node) -> Reach {
        self.reach[node.index()]
    }

    pub(crate) fn set_reach(&mut self, node: Node, reach: Reach) {
        self.reach[node.index()] = reach;
    }

    pub(crate) fn has(&self, node: Node, flag: u16) -> bool {
        self.flags[node.index()] & flag != 0
    }

    pub(crate) fn raise(&mut self, node: Node, flag: u16) {
        self.flags[node.index()] |= flag;
    }

    pub(crate) fn lower(&mut self, node: Node, flag: u16) {
        self.flags[node.index()] &= !flag;
    }

    pub(crate) fn flags(&self, node: Node) -> u16 {
        self.flags[node.index()]
    }

    pub(crate) fn mark(&mut self, layouts: &Layouts, node: Node, flag: u16) {
        mark_path(&mut self.flags, layouts, node, flag);
    }
}

fn mark_path(flags: &mut [u16], layouts: &Layouts, node: Node, flag: u16) {
    flags[node.index()] |= flag;
    let mut above = layouts.parent(node);
    while let Some(parent) = above {
        if flags[parent.index()] & BELOW != 0 {
            break;
        }
        flags[parent.index()] |= BELOW;
        above = layouts.parent(parent);
    }
}
