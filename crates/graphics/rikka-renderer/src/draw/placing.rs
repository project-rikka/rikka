use rikka_types::Rect;

use crate::draw::chunks::{CHUNK_INSTANCES, Chunk, Segment};
use crate::draw::order_positions::OrderPositions;
use crate::draw::retained::{RetainedBatch, UNPLACED};
use crate::scene::{INVISIBLE, Node, NodeId, Scene};

const LOOK_BACK: usize = 64;

impl RetainedBatch {
    pub(crate) fn place(&mut self, scene: &Scene, id: NodeId, positions: &OrderPositions) -> bool {
        let Some(node) = scene.get(id) else {
            return true;
        };
        let Some(span) = self.tracked_slot(id).map(|slot| slot.span) else {
            return true;
        };
        if span.room == 0 || hidden_in_layer(scene, node) {
            return true;
        }
        let Some(before) = self.predecessor(scene, id, node, positions) else {
            return false;
        };
        let bounds = drawn_bounds(scene, node);
        let textured = node.is_textured() && node.paints();
        let after = match before {
            Some(earlier) => match self.split_after(scene, earlier) {
                Some(at) => Some(at),
                None => return false,
            },
            None => None,
        };
        if let Some(at) = after {
            let chunk = &mut self.chunks[at];
            let follows = chunk.first + chunk.count == span.first
                && chunk.count + span.room <= CHUNK_INSTANCES
                && chunk.members.1 as usize == self.placed.len();
            if follows {
                chunk.count += span.room;
                chunk.members.1 += 1;
                chunk.bounds = joined(chunk.bounds, bounds);
                chunk.has_textured |= textured;
                let owner = chunk.id;
                self.placed.push(id);
                if let Some(slot) = self.tracked_slot(id) {
                    slot.chunk = owner;
                }
                return true;
            }
        }
        let member = self.placed.len() as u32;
        self.placed.push(id);
        let chunk = Chunk {
            first: span.first,
            count: span.room,
            bounds,
            members: (member, member + 1),
            id: self.next_chunk_id(),
            has_textured: textured,
        };
        if let Some(slot) = self.tracked_slot(id) {
            slot.chunk = chunk.id;
        }
        self.insert_chunk(after, chunk)
    }

    fn predecessor(
        &mut self,
        scene: &Scene,
        id: NodeId,
        node: &Node,
        positions: &OrderPositions,
    ) -> Option<Option<NodeId>> {
        let order = scene.settled_order();
        let at = if !scene.has_layers() {
            let rank = node.rank();
            let mut lost = false;
            let at = order.partition_point(|other| match scene.get(*other) {
                Some(other) => other.rank() < rank,
                None => {
                    lost = true;
                    false
                }
            });
            if lost || order.get(at) != Some(&id) {
                return None;
            }
            at
        } else {
            positions.find(order, id)?
        };
        for other in order[..at].iter().rev().take(LOOK_BACK) {
            let earlier = scene.get(*other)?;
            if earlier.backdrop.is_some() || earlier.layer != node.layer {
                return None;
            }
            if self
                .tracked_slot(*other)
                .is_some_and(|slot| slot.chunk != UNPLACED && slot.span.room > 0)
            {
                return Some(Some(*other));
            }
        }
        (at <= LOOK_BACK && node.layer.is_none()).then_some(None)
    }

    fn split_after(&mut self, scene: &Scene, earlier: NodeId) -> Option<usize> {
        let span = self.tracked_slot(earlier)?.span;
        let owner = self.tracked_slot(earlier)?.chunk;
        let at = self.chunk_index(owner)?;
        let chunk = self.chunks[at];
        let end = span.first + span.room;
        if span.first < chunk.first || end > chunk.first + chunk.count {
            return None;
        }
        let (from, to) = chunk.members;
        let within = self.placed[from as usize..to as usize]
            .iter()
            .position(|member| *member == earlier)? as u32;
        if end == chunk.first + chunk.count {
            return Some(at);
        }
        let parted = from + within + 1;
        let right = Chunk {
            first: end,
            count: chunk.first + chunk.count - end,
            bounds: Rect::ZERO,
            members: (parted, to),
            id: self.next_chunk_id(),
            has_textured: false,
        };
        self.chunks[at].count = end - chunk.first;
        self.chunks[at].members.1 = parted;
        for index in parted..to {
            let member = self.placed[index as usize];
            if let Some(slot) = self.tracked_slot(member)
                && slot.chunk == owner
            {
                slot.chunk = right.id;
            }
        }
        if !self.insert_chunk(Some(at), right) {
            return None;
        }
        let (left_bounds, left_textured) = self.chunk_summary(scene, at);
        self.chunks[at].bounds = left_bounds;
        self.chunks[at].has_textured = left_textured;
        let (right_bounds, right_textured) = self.chunk_summary(scene, at + 1);
        self.chunks[at + 1].bounds = right_bounds;
        self.chunks[at + 1].has_textured = right_textured;
        Some(at)
    }

    fn insert_chunk(&mut self, after: Option<usize>, chunk: Chunk) -> bool {
        let host = match after {
            Some(at) => self.segments.iter().position(|segment| {
                matches!(segment, Segment::Draw { first, count }
                    if (*first as usize..(*first + *count) as usize).contains(&at))
            }),
            None => self
                .segments
                .iter()
                .position(|segment| matches!(segment, Segment::Draw { .. })),
        };
        let at = match (after, host) {
            (Some(at), Some(_)) => at + 1,
            (None, Some(0)) => match self.segments[0] {
                Segment::Draw { first, .. } => first as usize,
                _ => return false,
            },
            (None, None) if self.segments.is_empty() && self.chunks.is_empty() => {
                self.segments.push(Segment::Draw { first: 0, count: 0 });
                0
            }
            _ => return false,
        };
        let host = host.unwrap_or(0);
        self.chunks.insert(at, chunk);
        for (index, segment) in self.segments.iter_mut().enumerate() {
            if let Segment::Draw { first, count } = segment {
                match index == host {
                    true => *count += 1,
                    false if *first as usize >= at => *first += 1,
                    false => {}
                }
            }
        }
        self.reindex();
        true
    }

    fn next_chunk_id(&mut self) -> u32 {
        let id = self.next_chunk;
        self.next_chunk += 1;
        id
    }
}

fn drawn_bounds(scene: &Scene, node: &Node) -> Rect {
    match node.paints() {
        true => scene.bounds_of(node),
        false => Rect::ZERO,
    }
}

fn hidden_in_layer(scene: &Scene, node: &Node) -> bool {
    node.layer.is_some()
        && (!node.paints()
            || scene
                .layer_chain(node.layer)
                .iter()
                .any(|(_, retained)| *retained <= INVISIBLE))
}

fn joined(all: Rect, bounds: Rect) -> Rect {
    match (all.is_empty(), bounds.is_empty()) {
        (true, _) => bounds,
        (_, true) => all,
        _ => all.union(&bounds),
    }
}
