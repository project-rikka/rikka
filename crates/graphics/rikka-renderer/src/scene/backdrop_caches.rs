use std::hash::{Hash, Hasher};

use rikka_types::Rect;
use rustc_hash::{FxHashMap, FxHasher};

use crate::fx::{BackdropCache, Plan, Spread, StaleCorners};
use crate::scene::audience::Viewer;
use crate::scene::damage::DamageTracker;
use crate::scene::node::NodeId;

const SCENE_KEY: u64 = 1 << 63;

#[derive(Debug, Default)]
struct ViewerCaches {
    caches: FxHashMap<NodeId, BackdropCache>,
    epochs: FxHashMap<NodeId, u32>,
}

impl ViewerCaches {
    fn begin(&mut self) {
        for cache in self.caches.values_mut() {
            cache.stale.clear();
        }
    }

    fn track(
        &mut self,
        scene: u64,
        era: u64,
        viewer: Viewer,
        id: NodeId,
        spread: &Spread,
        changed: &[Rect],
    ) -> &BackdropCache {
        let key = self.key(scene, era, viewer, id);
        let cache = self
            .caches
            .entry(id)
            .or_insert_with(|| BackdropCache::current(key));
        cache.key = key;
        cache.stale.clear();
        for rect in changed {
            spread.stale_regions(*rect, &mut cache.stale);
        }
        cache
    }

    fn extend_stale(&mut self, id: NodeId, extra: &[Rect]) {
        if let Some(cache) = self.caches.get_mut(&id) {
            cache.stale.extend_from_slice(extra);
        }
    }

    fn lost(&mut self, id: NodeId) {
        self.caches.remove(&id);
        *self.epochs.entry(id).or_default() += 1;
    }

    fn removed(&mut self, id: NodeId) {
        self.caches.remove(&id);
        self.epochs.remove(&id);
    }

    fn forget(&mut self) {
        self.caches.clear();
        self.epochs.clear();
    }

    fn key(&self, scene: u64, era: u64, viewer: Viewer, id: NodeId) -> u64 {
        let mut hasher = FxHasher::default();
        (scene, era, viewer.index() as u8, id.index, id.generation).hash(&mut hasher);
        self.epochs.get(&id).copied().unwrap_or(0).hash(&mut hasher);
        hasher.finish() | SCENE_KEY
    }
}

#[derive(Debug, Default)]
pub(super) struct BackdropCaches {
    era: u64,
    viewers: [ViewerCaches; Viewer::ALL.len()],
}

impl BackdropCaches {
    pub(super) fn begin(&mut self, viewer: Viewer) {
        self.viewers[viewer.index()].begin();
    }

    pub(super) fn track(
        &mut self,
        viewer: Viewer,
        scene: u64,
        id: NodeId,
        spread: &Spread,
        changed: &[Rect],
    ) -> &BackdropCache {
        self.viewers[viewer.index()].track(scene, self.era, viewer, id, spread, changed)
    }

    pub(super) fn extend_stale(&mut self, viewer: Viewer, id: NodeId, extra: &[Rect]) {
        self.viewers[viewer.index()].extend_stale(id, extra);
    }

    pub(super) fn lost(&mut self, viewer: Viewer, id: NodeId) {
        self.viewers[viewer.index()].lost(id);
    }

    pub(super) fn removed(&mut self, id: NodeId) {
        for caches in &mut self.viewers {
            caches.removed(id);
        }
    }

    pub(super) fn forget(&mut self) {
        self.era += 1;
        for caches in &mut self.viewers {
            caches.forget();
        }
    }

    pub(super) fn get(&self, viewer: Viewer, id: NodeId) -> Option<&BackdropCache> {
        self.viewers[viewer.index()].caches.get(&id)
    }

    pub(super) fn keys(&self) -> impl Iterator<Item = u64> + '_ {
        self.viewers
            .iter()
            .flat_map(|caches| caches.caches.values().map(|cache| cache.key))
    }
}

pub(crate) struct PreparedBackdrop {
    pub(crate) node: NodeId,
    pub(crate) plan: Plan,
    pub(crate) spread: Option<Spread>,
    pub(crate) reshaped: Option<StaleCorners>,
}

#[derive(Clone, Copy)]
pub(super) struct Zone {
    pub(super) entry: usize,
    pub(super) rect: Rect,
    pub(super) taken: bool,
}

#[derive(Default)]
pub(crate) struct FrameBackdrops {
    pub(crate) entries: Vec<PreparedBackdrop>,
    by_node: FxHashMap<NodeId, usize>,
    pub(super) zones: Vec<Zone>,
    pub(super) added: Vec<Rect>,
    pub(super) above: Vec<Rect>,
    pub(super) extra_stale: Vec<Rect>,
    pub(super) tracker: DamageTracker,
}

impl FrameBackdrops {
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.by_node.clear();
        self.zones.clear();
        self.added.clear();
        self.above.clear();
        self.extra_stale.clear();
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn push(&mut self, entry: PreparedBackdrop) -> usize {
        let index = self.entries.len();
        self.by_node.insert(entry.node, index);
        self.entries.push(entry);
        index
    }

    pub(crate) fn get(&self, id: NodeId) -> Option<&PreparedBackdrop> {
        let &index = self.by_node.get(&id)?;
        self.entries.get(index)
    }

    pub(super) fn add_zone(&mut self, entry: usize, rect: Rect) {
        self.zones.push(Zone {
            entry,
            rect,
            taken: false,
        });
    }

    pub(super) fn untake_zones(&mut self) {
        for zone in &mut self.zones {
            zone.taken = false;
        }
    }
}
