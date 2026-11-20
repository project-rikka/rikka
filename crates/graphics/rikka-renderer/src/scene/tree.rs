use rikka_types::{Point, Rect, RoundedRect};
use smallvec::SmallVec;

use crate::scene::dirt::SHIFTED;
use crate::scene::node::{GroupClip, GroupId, LayerId, Node, NodeId};
use crate::scene::scene_impl::Scene;
use crate::scene::slab::Slab;

pub(crate) const INVISIBLE: f32 = 0.5 / 255.0;

pub(super) const LAYER_DEPTH: usize = 16;
const MEMBERS_IN_DETAIL: usize = 64;

#[derive(Clone, Debug, Default)]
pub(super) struct Group {
    pub(super) parent: Option<GroupId>,
    pub(super) offset: Point,
    pub(super) shown: Rect,
    pub(super) clip: GroupClip,
    resolved: Point,
    resolved_clip: Option<RoundedRect>,
    children: Vec<GroupId>,
}

#[derive(Debug, Default)]
pub(super) struct Members {
    ids: Vec<NodeId>,
    stale: usize,
}

#[derive(Clone, Debug)]
pub(super) struct Layer {
    pub(super) parent: Option<LayerId>,
    pub(super) opacity: f32,
    bounds: Rect,
    chain: Vec<(LayerId, f32)>,
}

impl Layer {
    fn new() -> Self {
        Self {
            parent: None,
            opacity: 1.0,
            bounds: Rect::ZERO,
            chain: Vec::new(),
        }
    }
}

pub(super) fn fades(opacity: f32) -> bool {
    opacity > INVISIBLE && opacity < 1.0
}

impl Scene {
    pub fn create_group(&mut self) -> GroupId {
        let (index, generation) = self.groups.insert(Group::default());
        let id = GroupId { index, generation };
        self.journal.groups.push(id);
        self.mark_group_changed(index);
        id
    }

    pub fn set_group(
        &mut self,
        id: GroupId,
        parent: Option<GroupId>,
        offset: Point,
        shown: Rect,
    ) -> bool {
        let parent = parent.filter(|above| !self.within_group(*above, id));
        let Some(group) = self.groups.get(id.index, id.generation) else {
            return false;
        };
        let standing = (group.parent, group.offset, group.shown);
        if standing == (parent, offset, shown) {
            return false;
        }
        let moves = standing.1 != offset || standing.0 != parent;
        let was_shown = match moves {
            true => self.shown_of(id),
            false => Rect::ZERO,
        };
        let group = self
            .groups
            .get_mut(id.index, id.generation)
            .expect("entry exists");
        group.parent = parent;
        group.offset = offset;
        group.shown = shown;
        self.journal.groups.push(id);
        if standing.0 != parent {
            self.reparent(id, standing.0, parent);
        }
        if moves {
            let before = self.member_bounds(id);
            let unclipped_before = match before {
                Some(_) => None,
                None => Some(self.unclipped_member_bounds(id)),
            };
            self.resolve_from(id);
            match (before, self.member_bounds(id)) {
                (Some(before), Some(after)) => {
                    self.damage.add(before);
                    self.damage.add(after);
                }
                _ => {
                    self.damage.add(was_shown);
                    self.damage.add(self.shown_of(id));
                    if let Some(bounds) = unclipped_before {
                        self.damage.add(bounds);
                        self.damage.add(self.unclipped_member_bounds(id));
                    }
                }
            }
        }
        true
    }

    fn shown_of(&self, id: GroupId) -> Rect {
        let Some(group) = self.groups.get(id.index, id.generation) else {
            return Rect::ZERO;
        };
        let above = group
            .parent
            .and_then(|parent| self.groups.get(parent.index, parent.generation))
            .map_or(Point::ZERO, |parent| parent.resolved);
        let shown = group.shown.translate(above);
        match group.resolved_clip {
            Some(clip) => shown.intersection(&clip.rect).unwrap_or(Rect::ZERO),
            None => shown,
        }
    }

    pub fn set_group_clip(&mut self, id: GroupId, clip: GroupClip) -> bool {
        let Some(group) = self.groups.get_mut(id.index, id.generation) else {
            return false;
        };
        if group.clip == clip {
            return false;
        }
        group.clip = clip;
        self.journal.groups.push(id);
        self.resolve_from(id);
        true
    }

    pub(super) fn put_group(&mut self, id: GroupId) {
        if self.groups.get(id.index, id.generation).is_none() {
            self.groups.put(id.index, id.generation, Group::default());
            self.mark_group_changed(id.index);
            self.resolve_from(id);
        }
    }

    fn mark_group_changed(&mut self, index: u32) {
        self.journal.moved_groups.push(index + 1);
        self.journal.reclipped_groups.push(index + 1);
    }

    fn reparent(&mut self, id: GroupId, from: Option<GroupId>, to: Option<GroupId>) {
        if let Some(from) = from
            && let Some(parent) = self.groups.get_mut(from.index, from.generation)
        {
            parent.children.retain(|child| *child != id);
        }
        if let Some(to) = to
            && let Some(parent) = self.groups.get_mut(to.index, to.generation)
        {
            parent.children.push(id);
        }
    }

    pub fn remove_group(&mut self, id: GroupId) {
        let shown = self.shown_of(id);
        let before = self.member_bounds(id);
        let Some(group) = self.groups.remove(id.index, id.generation) else {
            return;
        };
        self.journal.gone_groups.push(id);
        match before {
            Some(bounds) => self.damage.add(bounds),
            None => self.damage.add(shown),
        }
        self.mark_group_changed(id.index);
        self.reparent(id, group.parent, None);
        self.shift_members(id);
        for child in group.children {
            self.resolve_from(child);
            if let Some(after) = self.member_bounds(child) {
                self.damage.add(after);
            } else {
                self.damage.add(self.shown_of(child));
            }
        }
    }

    pub fn create_layer(&mut self) -> LayerId {
        let (index, generation) = self.layers.insert(Layer::new());
        let id = LayerId { index, generation };
        self.journal.layers.push(id);
        self.layers_dirty = true;
        id
    }

    pub fn set_layer(&mut self, id: LayerId, parent: Option<LayerId>, opacity: f32) -> bool {
        let opacity = opacity.clamp(0.0, 1.0);
        let parent = parent.filter(|above| self.is_nested_in(*above, id));
        let Some(layer) = self.layers.get_mut(id.index, id.generation) else {
            return false;
        };
        if (layer.parent, layer.opacity) == (parent, opacity) {
            return false;
        }
        let (was, was_within, covered) = (layer.opacity, layer.parent, layer.bounds);
        layer.parent = parent;
        layer.opacity = opacity;
        self.faded = self.faded - usize::from(fades(was)) + usize::from(fades(opacity));
        if covered.is_empty() {
            let live = self.layer_member_bounds(id);
            self.damage.add(live);
        } else {
            self.damage.add(covered);
        }
        self.journal.layers.push(id);
        let shows = |opacity: f32| (fades(opacity), opacity <= INVISIBLE);
        if was_within != parent {
            self.order_dirty = true;
            self.journal.restacked_layers = true;
        } else if shows(was) != shows(opacity) {
            self.journal.restacked_layers = true;
        } else {
            self.journal.faded_layers = true;
        }
        self.layers_dirty = true;
        true
    }

    pub(super) fn put_layer(&mut self, id: LayerId) {
        if self.layers.get(id.index, id.generation).is_none() {
            self.layers.put(id.index, id.generation, Layer::new());
            self.layers_dirty = true;
        }
    }

    fn is_nested_in(&self, above: LayerId, id: LayerId) -> bool {
        let mut current = Some(above);
        for _ in 0..LAYER_DEPTH {
            let Some(at) = current else { return true };
            if at == id {
                return false;
            }
            let Some(layer) = self.layers.get(at.index, at.generation) else {
                return false;
            };
            current = layer.parent;
        }
        false
    }

    pub fn remove_layer(&mut self, id: LayerId) {
        let Some(layer) = self.layers.remove(id.index, id.generation) else {
            return;
        };
        self.journal.gone_layers.push(id);
        self.journal.restacked_layers = true;
        self.faded -= usize::from(fades(layer.opacity));
        self.damage.add(layer.bounds);
        self.order_dirty = true;
        self.layers_dirty = true;
    }

    pub(super) fn resolve_layers(&mut self) {
        for index in 0..self.layers.places() {
            let Some((at, generation)) = self.layers.id_at(index) else {
                continue;
            };
            let mut chain =
                std::mem::take(&mut self.layers.at_mut(index).expect("slot is occupied").chain);
            chain.clear();
            let mut current = Some(LayerId {
                index: at,
                generation,
            });
            for _ in 0..LAYER_DEPTH {
                let Some(id) = current else { break };
                let Some(layer) = self.layers.get(id.index, id.generation) else {
                    break;
                };
                chain.push((id, layer.opacity));
                current = layer.parent;
            }
            chain.reverse();
            self.layers.at_mut(index).expect("slot is occupied").chain = chain;
        }
    }

    fn layer_member_bounds(&self, target: LayerId) -> Rect {
        let mut bounds = Rect::ZERO;
        for (_, node) in self.nodes.iter() {
            if !node.paints() {
                continue;
            }
            let mut current = node.layer;
            let mut belongs = false;
            for _ in 0..LAYER_DEPTH {
                let Some(at) = current else { break };
                if at == target {
                    belongs = true;
                    break;
                }
                current = self
                    .layers
                    .get(at.index, at.generation)
                    .and_then(|layer| layer.parent);
            }
            if !belongs {
                continue;
            }
            extend_union(&mut bounds, self.bounds_of(node));
        }
        bounds
    }

    pub(crate) fn layer_chain(&self, layer: Option<LayerId>) -> &[(LayerId, f32)] {
        layer
            .and_then(|id| self.layers.get(id.index, id.generation))
            .map_or(&[], |layer| layer.chain.as_slice())
    }

    pub(crate) fn layer_opacity(&self, id: LayerId) -> Option<f32> {
        self.layers
            .get(id.index, id.generation)
            .map(|layer| layer.opacity)
    }

    pub(crate) fn set_layer_bounds(&mut self, id: LayerId, bounds: Rect) {
        if let Some(layer) = self.layers.get_mut(id.index, id.generation) {
            layer.bounds = bounds;
        }
    }

    pub(crate) fn widen_layer_bounds(&mut self, id: LayerId, bounds: Rect) -> bool {
        let Some(layer) = self.layers.get_mut(id.index, id.generation) else {
            return false;
        };
        let widened = match layer.bounds.is_empty() {
            true => bounds,
            false => layer.bounds.union(&bounds),
        };
        let grew = widened != layer.bounds;
        layer.bounds = widened;
        grew
    }

    pub(crate) fn layer_bounds(&self, id: LayerId) -> Option<Rect> {
        self.layers
            .get(id.index, id.generation)
            .map(|layer| layer.bounds)
    }

    pub(crate) fn has_fades(&self) -> bool {
        self.faded != 0
    }

    pub(crate) fn offset_of(&self, group: Option<GroupId>) -> Point {
        offset_of(&self.groups, group)
    }

    pub(crate) fn bounds_of(&self, node: &Node) -> Rect {
        bounds_of(&self.groups, node)
    }

    pub(crate) fn group_clip_of(&self, node: &Node) -> Option<RoundedRect> {
        group_clip_of(&self.groups, node)
    }

    pub(crate) fn group_slot(group: Option<GroupId>) -> u32 {
        group.map_or(0, |id| id.index + 1)
    }

    pub(crate) fn group_slots(&self) -> usize {
        self.groups.places() + 1
    }

    pub(crate) fn group_offset_at(&self, slot: usize) -> [f32; 2] {
        let offset = slot
            .checked_sub(1)
            .and_then(|index| self.groups.at(index))
            .map_or(Point::ZERO, |group| group.resolved);
        [offset.x, offset.y]
    }

    pub(crate) fn group_clip_at(&self, slot: usize) -> Option<RoundedRect> {
        slot.checked_sub(1)
            .and_then(|index| self.groups.at(index))
            .and_then(|group| group.resolved_clip)
    }

    fn resolve_from(&mut self, id: GroupId) {
        let mut pending = std::mem::take(&mut self.resolving);
        pending.clear();
        pending.push(id);
        while let Some(id) = pending.pop() {
            let Some(group) = self.groups.get(id.index, id.generation) else {
                continue;
            };
            let (above, bounded) = group
                .parent
                .and_then(|parent| self.groups.get(parent.index, parent.generation))
                .map_or((Point::ZERO, None), |parent| {
                    (parent.resolved, parent.resolved_clip)
                });
            let resolved = above + group.offset;
            let resolved_clip = resolve_clip(group.clip, above, bounded);
            let shown = group.shown.translate(above);
            let group = self
                .groups
                .get_mut(id.index, id.generation)
                .expect("entry exists");
            if group.resolved == resolved && group.resolved_clip == resolved_clip {
                continue;
            }
            let was_clip = std::mem::replace(&mut group.resolved_clip, resolved_clip);
            let moved = std::mem::replace(&mut group.resolved, resolved) != resolved;
            pending.extend(group.children.iter().copied());
            if was_clip != resolved_clip {
                let reshaped = was_clip
                    .zip(resolved_clip)
                    .and_then(|(was, now)| now.changed_corners(&was));
                match reshaped {
                    Some(corners) => self.damage.add_all(corners),
                    None => {
                        let fallback = match shown.is_empty() {
                            true => self.member_bounds(id).unwrap_or(shown),
                            false => shown,
                        };
                        let reach =
                            |clip: Option<RoundedRect>| clip.map_or(fallback, |clip| clip.rect);
                        self.damage.add(reach(was_clip));
                        self.damage.add(reach(resolved_clip));
                    }
                }
                self.journal.reclipped_groups.push(id.index + 1);
            }
            if moved {
                self.journal.moved_groups.push(id.index + 1);
            }
            self.shift_members(id);
        }
        self.resolving = pending;
    }
    fn within_group(&self, inner: GroupId, outer: GroupId) -> bool {
        let mut current = Some(inner);
        while let Some(at) = current {
            if at == outer {
                return true;
            }
            current = self
                .groups
                .get(at.index, at.generation)
                .and_then(|group| group.parent);
        }
        false
    }

    pub(super) fn add_member(&mut self, id: NodeId, group: Option<GroupId>) {
        let Some(group) = group else { return };
        let slot = group.index as usize;
        if self.grouped.len() <= slot {
            self.grouped.resize_with(slot + 1, Members::default);
        }
        self.grouped[slot].ids.push(id);
    }

    pub(super) fn remove_from_groups(&mut self, node: &Node) {
        self.remove_member(node.group);
        if node.clip_group != node.group {
            self.remove_member(node.clip_group);
        }
    }

    pub(super) fn move_clip(
        &mut self,
        id: NodeId,
        group: Option<GroupId>,
        from: Option<GroupId>,
        to: Option<GroupId>,
    ) {
        if from != group {
            self.remove_member(from);
        }
        if to != group {
            self.add_member(id, to);
        }
    }

    fn remove_member(&mut self, group: Option<GroupId>) {
        let Some(slot) = group.map(|group| group.index as usize) else {
            return;
        };
        let Some(members) = self.grouped.get_mut(slot) else {
            return;
        };
        members.stale += 1;
        if members.stale * 2 < members.ids.len() {
            return;
        }
        let mut members = std::mem::take(members);
        let holder = self
            .groups
            .id_at(slot)
            .map(|(index, generation)| GroupId { index, generation });
        members.ids.sort_unstable();
        members.ids.dedup();
        members
            .ids
            .retain(|id| holder.is_some_and(|holder| self.belongs(*id, holder)));
        members.stale = 0;
        self.grouped[slot] = members;
    }
    fn member_bounds(&self, id: GroupId) -> Option<Rect> {
        let mut pending: SmallVec<[GroupId; 8]> = SmallVec::new();
        pending.push(id);
        let mut seen = 0;
        let mut bounds = Rect::ZERO;
        while let Some(group) = pending.pop() {
            let entry = self.groups.get(group.index, group.generation)?;
            pending.extend(entry.children.iter().copied());
            let members = self.grouped.get(group.index as usize);
            for member in members.into_iter().flat_map(|members| &members.ids) {
                seen += 1;
                if seen > MEMBERS_IN_DETAIL {
                    return None;
                }
                let Some(node) = self
                    .nodes
                    .get(member.index, member.generation)
                    .filter(|node| node.paints())
                else {
                    continue;
                };
                extend_union(&mut bounds, self.bounds_of(node));
            }
        }
        Some(bounds)
    }

    fn unclipped_member_bounds(&self, id: GroupId) -> Rect {
        if self.unclipped.is_empty() {
            return Rect::ZERO;
        }
        let mut bounds = Rect::ZERO;
        for &member in &self.unclipped {
            let Some(node) = self
                .nodes
                .get(member.index, member.generation)
                .filter(|node| !node.clipped_by_group && node.paints())
            else {
                continue;
            };
            if node.group.is_some_and(|group| self.within_group(group, id)) {
                extend_union(&mut bounds, self.bounds_of(node));
            }
        }
        bounds
    }

    fn belongs(&self, id: NodeId, group: GroupId) -> bool {
        self.nodes
            .get(id.index, id.generation)
            .is_some_and(|node| node.group == Some(group) || node.clip_group == Some(group))
    }

    fn shift_members(&mut self, group: GroupId) {
        let slot = group.index as usize;
        let Some(mut members) = self.grouped.get_mut(slot).map(std::mem::take) else {
            return;
        };
        let before = members.ids.len();
        members.ids.retain(|id| {
            let belongs = self.belongs(*id, group);
            if belongs {
                self.mark(*id, SHIFTED);
            }
            belongs
        });
        members.stale = members.stale.saturating_sub(before - members.ids.len());
        if self.groups.get(group.index, group.generation).is_none() {
            members.ids.clear();
            members.stale = 0;
        }
        self.grouped[slot] = members;
    }
}

pub(super) fn offset_of(groups: &Slab<Group>, group: Option<GroupId>) -> Point {
    let Some(id) = group else {
        return Point::ZERO;
    };
    groups
        .get(id.index, id.generation)
        .map_or(Point::ZERO, |group| group.resolved)
}

pub(super) fn group_clip_of(groups: &Slab<Group>, node: &Node) -> Option<RoundedRect> {
    let id = node.group.filter(|_| node.clipped_by_group)?;
    groups
        .get(id.index, id.generation)
        .and_then(|group| group.resolved_clip)
}

pub(super) fn bounds_of(groups: &Slab<Group>, node: &Node) -> Rect {
    let bounds = node.paint_bounds(
        offset_of(groups, node.group),
        offset_of(groups, node.clip_group),
    );
    match group_clip_of(groups, node) {
        Some(clip) => bounds.intersection(&clip.rect).unwrap_or(Rect::ZERO),
        None => bounds,
    }
}

fn extend_union(bounds: &mut Rect, painted: Rect) {
    if painted.is_empty() {
        return;
    }
    *bounds = match bounds.is_empty() {
        true => painted,
        false => bounds.union(&painted),
    };
}

fn resolve_clip(
    clip: GroupClip,
    above: Point,
    bounded: Option<RoundedRect>,
) -> Option<RoundedRect> {
    let placed = |shape: RoundedRect| RoundedRect::new(shape.rect.translate(above), shape.radii);
    match clip {
        GroupClip::Inherit => bounded,
        GroupClip::Narrow(shape) => {
            let shape = placed(shape);
            Some(match bounded {
                Some(outer) => shape
                    .intersect(outer)
                    .unwrap_or(RoundedRect::rect(Rect::ZERO)),
                None => shape,
            })
        }
        GroupClip::Only(shape) => shape.map(placed),
    }
}
