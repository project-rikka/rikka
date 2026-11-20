use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rikka_types::{Color, Rect};
use rustc_hash::FxHashMap;

use crate::draw::SceneCache;
use crate::fx::{BackdropCache, Optics, Plan, PostEffect, Spread, StaleCorners};
use crate::scene::audience::Viewer;
use crate::scene::backdrop_caches::{BackdropCaches, FrameBackdrops, PreparedBackdrop};
use crate::scene::damage::{DamagePolicy, ViewerDamage};
use crate::scene::dirt::{BOUNDS, CONTENT, Changes, Dirt, Journal, NEW, ORDER, Reset};
use crate::scene::frame::FrameTracking;
use crate::scene::node::{GroupId, LayerId, Node, NodeId, Rank, Shape};
use crate::scene::order::{Member, Sorting};
use crate::scene::slab::Slab;
use crate::scene::tree::{Group, Layer, Members, bounds_of};
use rikka_gpu::texture::TextureId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct SceneId(u64);

#[must_use]
pub struct Scene {
    id: SceneId,
    pub(super) nodes: Slab<Node>,
    pub(super) groups: Slab<Group>,
    pub(super) order: Vec<NodeId>,
    pub(super) order_dirty: bool,
    pub(super) order_pruned: bool,
    pub(super) order_lost: bool,
    pub(super) damage: ViewerDamage,
    next_seq: u64,
    pub(super) effects: Vec<Arc<dyn PostEffect>>,
    pub(super) background: Color,
    backdrops: Vec<NodeId>,
    pub(super) unclipped: Vec<NodeId>,
    by_texture: FxHashMap<TextureId, Vec<NodeId>>,
    pub(super) layers: Slab<Layer>,
    pub(super) layers_dirty: bool,
    pub(super) faded: usize,
    pub(super) members: Vec<Vec<(Rank, Member)>>,
    pub(super) planes: Vec<(LayerId, Rank)>,
    pub(super) sorting: Sorting,
    pub(super) journal: Journal,
    pub(super) grouped: Vec<Members>,
    pub(super) tracking: FrameTracking,
    pub(super) resolving: Vec<GroupId>,
    backdrop_caches: BackdropCaches,
    pub(crate) cache: Option<Box<SceneCache>>,
}

impl Scene {
    pub fn new() -> Self {
        Self::with_damage(DamagePolicy::Merged)
    }

    pub fn immediate() -> Self {
        Self::with_damage(DamagePolicy::Exact)
    }

    fn with_damage(policy: DamagePolicy) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self {
            id: SceneId(NEXT.fetch_add(1, Ordering::Relaxed)),
            nodes: Slab::new(),
            groups: Slab::new(),
            order: Vec::new(),
            order_dirty: false,
            order_pruned: false,
            order_lost: false,
            damage: ViewerDamage::new(policy),
            next_seq: 0,
            effects: Vec::new(),
            background: Color::TRANSPARENT,
            backdrops: Vec::new(),
            unclipped: Vec::new(),
            by_texture: FxHashMap::default(),
            layers: Slab::new(),
            layers_dirty: false,
            faded: 0,
            members: Vec::new(),
            planes: Vec::new(),
            sorting: Sorting::default(),
            journal: Journal::default(),
            grouped: Vec::new(),
            tracking: FrameTracking::default(),
            resolving: Vec::new(),
            backdrop_caches: BackdropCaches::default(),
            cache: None,
        }
    }

    pub(crate) fn id(&self) -> SceneId {
        self.id
    }

    pub fn set_background(&mut self, background: Color) {
        if self.background != background {
            self.background = background;
            self.journal.background = true;
            self.damage.mark_full();
        }
    }

    pub fn background(&self) -> Color {
        self.background
    }

    pub fn insert(&mut self, mut node: Node) -> NodeId {
        node.seq = self.next_seq;
        self.next_seq += 1;
        if node.visible {
            self.damage.add_for(node.audience, self.bounds_of(&node));
        }
        node.dirt = 0;
        let (index, generation) = self.nodes.insert(node);
        let id = NodeId { index, generation };
        self.track_node(id, NEW | CONTENT | BOUNDS | ORDER, true);
        id
    }

    pub(super) fn track_node(&mut self, id: NodeId, dirt: Dirt, fresh: bool) {
        let Some(node) = self.nodes.get_mut(id.index, id.generation) else {
            return;
        };
        let before = node.dirt;
        node.dirt |= dirt;
        let (texture, backdrop) = (texture_of(node), node.backdrop.is_some());
        let (group, clip_group, unclipped) =
            (node.group, node.clip_group, node.escapes_group_clip());
        if before == 0 && dirt != 0 {
            self.journal.nodes.push(id);
        }
        if dirt & (NEW | ORDER) != 0 {
            self.rerank(id);
        }
        if fresh {
            if let Some(texture) = texture {
                self.add_texture_use(texture, id);
            }
            if backdrop {
                self.backdrops.push(id);
            }
            if unclipped {
                self.unclipped.push(id);
            }
            self.add_member(id, group);
            if clip_group != group {
                self.add_member(id, clip_group);
            }
        }
    }

    pub(super) fn add_texture_use(&mut self, texture: TextureId, id: NodeId) {
        self.by_texture.entry(texture).or_default().push(id);
    }

    pub fn remove(&mut self, id: NodeId) -> bool {
        let Some(bounds) = self.get(id).map(|node| self.bounds_of(node)) else {
            return false;
        };
        let node = self
            .nodes
            .remove(id.index, id.generation)
            .expect("node exists");
        if node.visible {
            self.damage.add_for(node.audience, bounds);
        }
        self.untrack(id, &node);
        self.remove_from_groups(&node);
        self.journal.removed.push(id);
        if node.layer.is_some() {
            self.order_dirty = true;
        }
        self.order_pruned = true;
        true
    }

    pub(super) fn untrack(&mut self, id: NodeId, node: &Node) {
        if let Some(texture) = texture_of(node) {
            self.forget_texture_use(texture, id);
        }
        if node.backdrop.is_some() {
            self.forget_backdrop(id);
        }
        if node.escapes_group_clip() {
            self.count_unclipped(id, true, false);
        }
    }

    fn forget_backdrop(&mut self, id: NodeId) {
        if let Some(pos) = self.backdrops.iter().position(|backdrop| *backdrop == id) {
            self.backdrops.swap_remove(pos);
        }
        self.backdrop_caches.removed(id);
    }

    pub(super) fn count_unclipped(&mut self, id: NodeId, had: bool, has: bool) {
        match (had, has) {
            (false, true) => self.unclipped.push(id),
            (true, false) => {
                if let Some(pos) = self.unclipped.iter().position(|node| *node == id) {
                    self.unclipped.swap_remove(pos);
                }
            }
            _ => {}
        }
    }

    pub(super) fn mark(&mut self, id: NodeId, dirt: Dirt) {
        let Some(node) = self.nodes.get_mut(id.index, id.generation) else {
            return;
        };
        if node.dirt == 0 {
            self.journal.nodes.push(id);
        }
        node.dirt |= dirt;
    }

    pub(super) fn forget_texture_use(&mut self, texture: TextureId, id: NodeId) {
        let Some(users) = self.by_texture.get_mut(&texture) else {
            return;
        };
        if let Some(pos) = users.iter().position(|used| *used == id) {
            users.swap_remove(pos);
        }
        if users.is_empty() {
            self.by_texture.remove(&texture);
        }
    }

    pub(super) fn replace_texture(&mut self, id: NodeId, old: TextureId, new: TextureId) {
        if old == new {
            return;
        }
        self.forget_texture_use(old, id);
        self.by_texture.entry(new).or_default().push(id);
    }

    pub(super) fn count_backdrop(&mut self, id: NodeId, had: bool, has: bool) {
        match (had, has) {
            (false, true) => self.backdrops.push(id),
            (true, false) => self.forget_backdrop(id),
            _ => {}
        }
    }

    pub fn clear(&mut self) {
        self.forget_everything(Reset::Cleared);
        self.damage.mark_full();
    }

    pub fn restart(&mut self) {
        self.forget_everything(Reset::Restarted);
        self.damage.reset();
    }

    pub(super) fn forget_everything(&mut self, reset: Reset) {
        self.nodes.clear();
        self.groups.clear();
        self.layers.clear();
        self.layers_dirty = false;
        self.faded = 0;
        self.reset_order();
        self.backdrops.clear();
        self.unclipped.clear();
        self.backdrop_caches.forget();
        self.by_texture.clear();
        self.grouped.clear();
        self.journal.restart(reset);
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.index, id.generation)
    }

    pub fn prefetch(&self, id: NodeId) {
        self.nodes.prefetch(id.index);
    }

    pub fn painted_over(&self, id: NodeId, area: Rect) -> Option<Rect> {
        let Some(beneath) = self.get(id) else {
            return Some(area);
        };
        let rank = beneath.rank();
        let layered = beneath.layer.is_some();
        self.nodes
            .iter()
            .filter(|((index, generation), node)| {
                (*index, *generation) != (id.index, id.generation)
                    && node.paints()
                    && (layered || node.layer.is_some() || node.rank() > rank)
            })
            .filter_map(|(_, node)| self.bounds_of(node).intersection(&area))
            .reduce(|covered, more| covered.union(&more))
    }

    pub fn damage_node(&mut self, id: NodeId) -> bool {
        let Some((audience, bounds)) = self
            .get(id)
            .map(|node| (node.audience, self.bounds_of(node)))
        else {
            return false;
        };
        self.damage.add_for(audience, bounds);
        true
    }

    #[must_use]
    pub fn is_animating(&self) -> bool {
        self.cache
            .as_ref()
            .is_some_and(|cache| !cache.zooms.settling().is_empty())
    }

    pub(crate) fn restage_settling(&mut self) {
        let Some(cache) = self.cache.take() else {
            return;
        };
        for &id in cache.zooms.settling() {
            self.mark(id, CONTENT);
            self.damage_node(id);
        }
        self.cache = Some(cache);
    }

    pub fn notify_texture_updated(&mut self, texture: TextureId) {
        let Some(users) = self.by_texture.get(&texture) else {
            return;
        };
        let Scene {
            nodes,
            groups,
            damage,
            ..
        } = self;
        for id in users {
            let Some(node) = nodes.get(id.index, id.generation) else {
                continue;
            };
            if !node.visible {
                continue;
            }
            damage.add_for(node.audience, bounds_of(groups, node));
        }
    }

    pub(crate) fn has_textures(&self) -> bool {
        !self.by_texture.is_empty()
    }

    pub(crate) fn has_layers(&self) -> bool {
        self.layers.len() != 0
    }

    pub(crate) fn textured(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.by_texture.values().flatten().copied()
    }

    pub(crate) fn texture_uses(&self) -> impl Iterator<Item = (TextureId, &[NodeId])> + '_ {
        self.by_texture
            .iter()
            .map(|(texture, users)| (*texture, users.as_slice()))
    }

    pub(crate) fn uses_texture(&self, texture: TextureId) -> bool {
        self.by_texture.contains_key(&texture)
    }

    pub fn damage_all(&mut self) {
        self.damage.mark_full();
    }

    pub fn damage_rect(&mut self, rect: Rect) {
        self.damage.add(rect);
    }

    pub fn set_effects(&mut self, effects: Vec<Box<dyn PostEffect>>) {
        self.effects = effects.into_iter().map(Arc::from).collect();
        self.journal.effects = true;
        self.damage.mark_full();
    }

    pub fn effects(&self) -> &[Arc<dyn PostEffect>] {
        &self.effects
    }

    pub fn has_damage(&self) -> bool {
        self.has_damage_for(Viewer::Screen)
    }

    pub(crate) fn has_damage_for(&self, viewer: Viewer) -> bool {
        !self.damage.is_empty_for(viewer) || self.order_dirty
    }

    pub fn has_changes(&self) -> bool {
        self.has_damage() || !self.journal.is_empty()
    }

    pub(crate) fn has_edits(&self) -> bool {
        self.order_dirty || !self.journal.is_empty()
    }

    pub(crate) fn damage_covers(&mut self, viewer: Viewer, viewport: Rect) -> bool {
        self.damage.covers(viewer, viewport)
    }

    pub(crate) fn damage_whole_for(&mut self, viewer: Viewer) {
        self.damage.mark_full_for(viewer);
    }

    pub(crate) fn has_backdrops(&self) -> bool {
        !self.backdrops.is_empty()
    }

    fn prepare_backdrops(
        &mut self,
        viewer: Viewer,
        damage: &[Rect],
        viewport: Rect,
        backdrops: &mut FrameBackdrops,
        stale_corners: &dyn Fn(&Plan, &BackdropCache) -> Option<StaleCorners>,
    ) {
        backdrops.clear();
        for index in 0..self.backdrops.len() {
            let id = self.backdrops[index];
            let Some((optics, kept_by_owner)) = self.backdrop_optics(viewer, id, viewport) else {
                self.backdrop_caches.lost(viewer, id);
                continue;
            };
            let spread = (!kept_by_owner).then(|| Spread::from(&optics));
            let plan = optics.plan(viewport);
            let cache = match spread {
                Some(spread) => self
                    .backdrop_caches
                    .track(viewer, self.id.0, id, &spread, damage),
                None => self
                    .nodes
                    .get(id.index, id.generation)
                    .and_then(|node| node.backdrop_cache.as_deref())
                    .expect("owner-kept backdrop has a cache"),
            };
            let reshaped = stale_corners(&plan, cache);
            let entry = backdrops.entries.len();
            match reshaped {
                None => backdrops.add_zone(entry, plan.region),
                Some(corners) => {
                    for stale in cache.stale.iter().chain(corners.iter()) {
                        if let Some(zone) = plan.stale_zone(*stale) {
                            backdrops.add_zone(entry, zone);
                        }
                    }
                }
            }
            backdrops.push(PreparedBackdrop {
                node: id,
                plan,
                spread,
                reshaped,
            });
        }
    }

    fn propagate_backdrop_damage(
        &mut self,
        viewer: Viewer,
        damage: &mut Vec<Rect>,
        viewport: Rect,
        backdrops: &mut FrameBackdrops,
        track_implicit: bool,
    ) {
        if backdrops.zones.is_empty() {
            return;
        }
        backdrops.tracker.set_policy(self.damage.policy());
        loop {
            backdrops.added.clear();
            for index in 0..backdrops.zones.len() {
                let zone = backdrops.zones[index];
                if zone.taken || !damage.iter().any(|rect| rect.intersects(&zone.rect)) {
                    continue;
                }
                backdrops.zones[index].taken = true;
                let id = backdrops.entries[zone.entry].node;
                let before = backdrops.added.len();
                self.redrawn_for(id, zone.rect, &mut backdrops.above, &mut backdrops.added);
                if track_implicit && backdrops.added.len() > before {
                    self.stale_other_backdrops(viewer, backdrops, zone.entry, before);
                }
            }
            if backdrops.added.is_empty() {
                return;
            }
            backdrops.tracker.reset();
            backdrops
                .tracker
                .add_all(damage.drain(..).chain(backdrops.added.iter().copied()));
            backdrops.tracker.take_into(viewport, damage);
        }
    }

    fn stale_other_backdrops(
        &mut self,
        viewer: Viewer,
        backdrops: &mut FrameBackdrops,
        source: usize,
        added_from: usize,
    ) {
        for other in 0..backdrops.entries.len() {
            if other == source {
                continue;
            }
            let Some(spread) = backdrops.entries[other].spread else {
                continue;
            };
            backdrops.extra_stale.clear();
            for rect in &backdrops.added[added_from..] {
                spread.stale_regions(*rect, &mut backdrops.extra_stale);
            }
            if backdrops.extra_stale.is_empty() {
                continue;
            }
            let other_id = backdrops.entries[other].node;
            let existing = self
                .backdrop_caches
                .get(viewer, other_id)
                .map_or(&[][..], |cache| cache.stale.as_slice());
            backdrops
                .extra_stale
                .retain(|rect| !existing.iter().any(|held| held.contains_rect(rect)));
            if backdrops.extra_stale.is_empty() {
                continue;
            }
            self.backdrop_caches
                .extend_stale(viewer, other_id, &backdrops.extra_stale);
            if backdrops.entries[other].reshaped.is_none() {
                continue;
            }
            for index in 0..backdrops.extra_stale.len() {
                let stale = backdrops.extra_stale[index];
                let Some(zone) = backdrops.entries[other].plan.stale_zone(stale) else {
                    continue;
                };
                let covered = backdrops
                    .zones
                    .iter()
                    .any(|held| held.entry == other && held.rect.contains_rect(&zone));
                if !covered {
                    backdrops.add_zone(other, zone);
                }
            }
        }
    }

    fn expand_damage_for_backdrops(
        &mut self,
        viewer: Viewer,
        damage: &mut Vec<Rect>,
        viewport: Rect,
        backdrops: &mut FrameBackdrops,
        stale_corners: &dyn Fn(&Plan, &BackdropCache) -> Option<StaleCorners>,
    ) {
        backdrops.clear();
        self.backdrop_caches.begin(viewer);
        if damage.is_empty() || self.backdrops.is_empty() || self.is_immediate() {
            return;
        }
        self.prepare_backdrops(viewer, damage, viewport, backdrops, stale_corners);
        self.propagate_backdrop_damage(viewer, damage, viewport, backdrops, true);
    }

    pub(crate) fn expand_buffer_for_backdrops(
        &mut self,
        viewer: Viewer,
        damage: &mut Vec<Rect>,
        viewport: Rect,
        backdrops: &mut FrameBackdrops,
        stale_corners: &dyn Fn(&Plan, &BackdropCache) -> Option<StaleCorners>,
    ) {
        if damage.is_empty() || self.backdrops.is_empty() || self.is_immediate() {
            return;
        }
        if backdrops.is_empty() {
            self.prepare_backdrops(viewer, &[], viewport, backdrops, stale_corners);
        } else {
            backdrops.untake_zones();
        }
        self.propagate_backdrop_damage(viewer, damage, viewport, backdrops, false);
    }

    fn redrawn_for(&self, id: NodeId, zone: Rect, above: &mut Vec<Rect>, into: &mut Vec<Rect>) {
        above.clear();
        let retained = self
            .cache
            .as_ref()
            .and_then(|cache| cache.retained.as_ref());
        let (Some(node), Some(retained)) = (self.get(id), retained) else {
            into.push(zone);
            return;
        };
        if !retained.painted_above(self, id, above) {
            into.push(zone);
            return;
        }
        above.push(self.bounds_of(node));
        into.extend(above.iter().filter_map(|rect| rect.intersection(&zone)));
    }

    fn backdrop_optics(
        &self,
        viewer: Viewer,
        id: NodeId,
        viewport: Rect,
    ) -> Option<(Optics, bool)> {
        let node = self.get(id)?;
        let offset = self.offset_of(node.group);
        if !node.seen_by(viewer) {
            return None;
        }
        node.rect.translate(offset).intersection(&viewport)?;
        Some((node.recipe(offset)?.optics(), node.backdrop_cache.is_some()))
    }

    pub(crate) fn backdrop_cache(&self, viewer: Viewer, id: NodeId) -> Option<&BackdropCache> {
        self.get(id)?
            .backdrop_cache
            .as_deref()
            .or_else(|| self.backdrop_caches.get(viewer, id))
    }

    pub(crate) fn kept_blur_keys(&self) -> impl Iterator<Item = u64> + '_ {
        self.backdrop_caches.keys()
    }

    pub(crate) fn take_damage_into(
        &mut self,
        viewer: Viewer,
        viewport: Rect,
        damage: &mut Vec<Rect>,
        backdrops: &mut FrameBackdrops,
        stale_corners: &dyn Fn(&Plan, &BackdropCache) -> Option<StaleCorners>,
    ) {
        self.settle_order();
        self.damage.take_into(viewer, viewport, damage);
        self.expand_damage_for_backdrops(viewer, damage, viewport, backdrops, stale_corners);
    }

    pub(crate) fn take_changes(&mut self, into: &mut Changes) {
        self.settle_order();
        into.clear();
        for id in &self.journal.nodes {
            if let Some(node) = self.nodes.get_mut(id.index, id.generation)
                && node.dirt != 0
            {
                into.nodes.push((*id, std::mem::take(&mut node.dirt)));
            }
        }
        let journal = &mut self.journal;
        std::mem::swap(&mut into.removed, &mut journal.removed);
        std::mem::swap(&mut into.moved_groups, &mut journal.moved_groups);
        std::mem::swap(&mut into.reclipped_groups, &mut journal.reclipped_groups);
        into.restacked_layers = journal.restacked_layers;
        into.faded_layers = journal.faded_layers;
        into.resequenced = journal.resequenced;
        into.reset = journal.reset.is_some();
        journal.clear();
    }

    pub(crate) fn is_immediate(&self) -> bool {
        self.damage.policy() == DamagePolicy::Exact
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scene")
            .field("nodes", &self.nodes.len())
            .field("effects", &self.effects.len())
            .finish_non_exhaustive()
    }
}

pub(super) fn texture_of(node: &Node) -> Option<TextureId> {
    match node.shape {
        Shape::Image { texture, .. } => Some(texture),
        _ => None,
    }
}

const _: () = {
    const fn travels<T: Send>() {}
    travels::<Scene>();
    travels::<crate::Renderer>();
    travels::<crate::Recorded>();
};
