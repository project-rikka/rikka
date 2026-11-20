use crate::scene::dirt::{LAYER, NEW};
use crate::scene::node::{LayerId, NodeId, Rank};
use crate::scene::scene_impl::Scene;
use crate::scene::tree::LAYER_DEPTH;

type Plane = (i32, u64);

const ABSENT: u32 = u32::MAX;

#[derive(Clone, Copy, Debug)]
pub(super) enum Member {
    Node(NodeId),
    Layer(LayerId),
}

#[derive(Debug, Default)]
pub(crate) struct Sorting {
    keyed: Vec<(Rank, NodeId)>,
    reranked: Vec<NodeId>,
    picked: Vec<bool>,
    flat: bool,
    next: Vec<NodeId>,
    stood: Vec<u32>,
    ranked: Vec<(u32, NodeId)>,
    tails: Vec<u32>,
    before: Vec<u32>,
    stays: Vec<bool>,
}

impl Scene {
    pub(crate) fn settled_order(&self) -> &[NodeId] {
        &self.order
    }

    pub(crate) fn settle_order(&mut self) {
        if self.layers_dirty {
            self.resolve_layers();
            self.layers_dirty = false;
        }
        if self.order_dirty || self.order_lost {
            self.reorder();
        } else if self.order_pruned {
            let nodes = &self.nodes;
            self.order
                .retain(|id| nodes.get(id.index, id.generation).is_some());
            self.order_pruned = false;
        }
    }

    pub(super) fn rerank(&mut self, id: NodeId) {
        self.order_dirty = true;
        self.sorting.reranked.push(id);
    }

    pub(super) fn clear_order(&mut self) {
        self.order.clear();
        self.order_dirty = false;
        self.order_pruned = false;
        self.order_lost = true;
        self.sorting.reranked.clear();
        self.sorting.flat = false;
    }

    pub(super) fn reset_order(&mut self) {
        self.order.clear();
        self.order_dirty = false;
        self.order_pruned = false;
        self.order_lost = false;
        self.sorting.reranked.clear();
        self.sorting.flat = false;
    }

    fn reorder(&mut self) {
        let mut next = std::mem::take(&mut self.sorting.next);
        next.clear();
        let arrivals = self.layers.len() == 0
            && !self.sorting.reranked.is_empty()
            && self.sorting.reranked.iter().all(|id| {
                self.nodes
                    .get(id.index, id.generation)
                    .is_none_or(|node| node.dirt & NEW != 0)
            });
        match self.layers.len() {
            0 => self.order_flat(&mut next),
            _ => self.order_layered(&mut next),
        }
        self.sorting.reranked.clear();
        match self.order_lost {
            true => self.journal.resequenced = true,
            false => {
                if !arrivals {
                    self.damage_reordered(&next);
                }
                self.damage_restacked();
            }
        }
        std::mem::swap(&mut self.order, &mut next);
        self.sorting.next = next;
        self.order_dirty = false;
        self.order_pruned = false;
        self.order_lost = false;
    }

    fn damage_reordered(&mut self, next: &[NodeId]) {
        let mut sorting = std::mem::take(&mut self.sorting);
        let Sorting {
            stood,
            ranked,
            tails,
            before,
            stays,
            ..
        } = &mut sorting;
        if stood.len() < self.nodes.places() {
            stood.resize(self.nodes.places(), ABSENT);
        }
        for (at, id) in self.order.iter().enumerate() {
            if let Some(slot) = stood.get_mut(id.index as usize) {
                *slot = at as u32;
            }
        }
        ranked.clear();
        for id in next {
            let at = stood.get(id.index as usize).copied().unwrap_or(ABSENT);
            if at != ABSENT && self.order[at as usize] == *id {
                ranked.push((at, *id));
            }
        }
        for id in &self.order {
            if let Some(slot) = stood.get_mut(id.index as usize) {
                *slot = ABSENT;
            }
        }

        if !ranked.is_sorted_by_key(|(at, _)| *at) {
            longest_rising(ranked, tails, before, stays);
            self.journal.resequenced = true;
            for (index, (_, id)) in ranked.iter().enumerate() {
                if stays[index] {
                    continue;
                }
                if let Some(node) = self.nodes.get(id.index, id.generation) {
                    let bounds = self.bounds_of(node);
                    self.damage.add_for(node.audience, bounds);
                }
            }
        }
        self.sorting = sorting;
    }

    fn damage_restacked(&mut self) {
        for id in &self.journal.nodes {
            let Some(node) = self.nodes.get(id.index, id.generation) else {
                continue;
            };
            if node.dirt & LAYER != 0 {
                self.damage.add_for(node.audience, self.bounds_of(node));
                self.journal.restacked_layers = true;
            }
        }
    }

    fn order_flat(&mut self, next: &mut Vec<NodeId>) {
        match self.sorting.flat && !self.order_lost {
            true => self.merge_reranked(next),
            false => self.sort_whole(next),
        }
        self.sorting.flat = true;
    }

    fn sort_whole(&mut self, next: &mut Vec<NodeId>) {
        let keyed = &mut self.sorting.keyed;
        keyed.clear();
        keyed.extend(
            self.nodes
                .iter()
                .map(|((index, generation), node)| (node.rank(), NodeId { index, generation })),
        );
        if !keyed.is_sorted_by_key(|(rank, _)| *rank) {
            keyed.sort_unstable_by_key(|(rank, _)| *rank);
        }
        next.extend(keyed.iter().map(|(_, id)| *id));
    }

    fn merge_reranked(&mut self, next: &mut Vec<NodeId>) {
        let Sorting {
            keyed,
            reranked,
            picked,
            ..
        } = &mut self.sorting;
        if picked.len() < self.nodes.places() {
            picked.resize(self.nodes.places(), false);
        }
        keyed.clear();
        for id in reranked.drain(..) {
            if let Some(node) = self.nodes.get(id.index, id.generation)
                && !std::mem::replace(&mut picked[id.index as usize], true)
            {
                keyed.push((node.rank(), id));
            }
        }
        keyed.sort_unstable_by_key(|(rank, _)| *rank);
        let mut placed = keyed.iter().peekable();
        for id in &self.order {
            let Some(node) = self.nodes.get(id.index, id.generation) else {
                continue;
            };
            if picked[id.index as usize] {
                continue;
            }
            let rank = node.rank();
            while let Some((_, earlier)) = placed.next_if(|(first, _)| *first < rank) {
                next.push(*earlier);
            }
            next.push(*id);
        }
        next.extend(placed.map(|(_, id)| *id));
        for (_, id) in keyed.iter() {
            picked[id.index as usize] = false;
        }
    }

    fn order_layered(&mut self, next: &mut Vec<NodeId>) {
        let places = self.layers.places();
        let mut members = std::mem::take(&mut self.members);
        let mut planes = std::mem::take(&mut self.planes);
        for bucket in members.iter_mut() {
            bucket.clear();
        }
        members.resize_with(places + 1, Vec::new);
        planes.clear();

        for ((index, generation), node) in self.nodes.iter() {
            let rank = node.rank();
            let bucket = self.bucket_of(node.layer);
            members[bucket].push((rank, Member::Node(NodeId { index, generation })));
            if let Some(layer) = node.layer.filter(|_| bucket != 0) {
                planes.push((layer, rank));
            }
        }
        lowest_per_plane(&mut planes);
        for at in 0..planes.len() {
            let (layer, rank) = planes[at];
            let chain = self.layer_chain(Some(layer));
            let above = &chain[..chain.len().saturating_sub(1)];
            planes.extend(above.iter().map(|(outer, _)| (*outer, rank)));
        }
        lowest_per_plane(&mut planes);
        for (layer, rank) in &planes {
            let parent = self
                .layers
                .get(layer.index, layer.generation)
                .and_then(|layer| layer.parent);
            members[self.bucket_of(parent)].push((*rank, Member::Layer(*layer)));
        }

        for bucket in members.iter_mut() {
            bucket.sort_unstable_by_key(|(rank, _)| *rank);
        }
        flatten(&members, 0, None, 0, next);

        self.members = members;
        self.planes = planes;
        self.sorting.flat = false;
    }

    fn bucket_of(&self, layer: Option<LayerId>) -> usize {
        layer
            .filter(|id| self.layers.get(id.index, id.generation).is_some())
            .map_or(0, |id| id.index as usize + 1)
    }
}

fn plane_of(rank: Rank) -> Plane {
    (rank.0, rank.1)
}

fn lowest_per_plane(planes: &mut Vec<(LayerId, Rank)>) {
    planes.sort_unstable_by_key(|(layer, rank)| (*layer, *rank));
    planes.dedup_by(|later, first| later.0 == first.0 && plane_of(later.1) == plane_of(first.1));
}

fn flatten(
    members: &[Vec<(Rank, Member)>],
    bucket: usize,
    plane: Option<Plane>,
    depth: usize,
    out: &mut Vec<NodeId>,
) {
    if depth > LAYER_DEPTH {
        return;
    }
    let entries = members[bucket].as_slice();
    let entries = match plane {
        Some(plane) => {
            let start = entries.partition_point(|(rank, _)| plane_of(*rank) < plane);
            let rest = &entries[start..];
            &rest[..rest.partition_point(|(rank, _)| plane_of(*rank) == plane)]
        }
        None => entries,
    };
    for (rank, member) in entries {
        match member {
            Member::Node(id) => out.push(*id),
            Member::Layer(id) => flatten(
                members,
                id.index as usize + 1,
                Some(plane_of(*rank)),
                depth + 1,
                out,
            ),
        }
    }
}

fn longest_rising(
    ranked: &[(u32, NodeId)],
    tails: &mut Vec<u32>,
    before: &mut Vec<u32>,
    stays: &mut Vec<bool>,
) {
    tails.clear();
    before.clear();
    before.resize(ranked.len(), ABSENT);
    for (index, (at, _)) in ranked.iter().enumerate() {
        let reach = tails.partition_point(|tail| ranked[*tail as usize].0 < *at);
        if reach > 0 {
            before[index] = tails[reach - 1];
        }
        match tails.get_mut(reach) {
            Some(tail) => *tail = index as u32,
            None => tails.push(index as u32),
        }
    }
    stays.clear();
    stays.resize(ranked.len(), false);
    let mut at = tails.last().copied().unwrap_or(ABSENT);
    while at != ABSENT {
        stays[at as usize] = true;
        at = before[at as usize];
    }
}
