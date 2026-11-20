use rikka_types::{Point, Rect};
use rustc_hash::FxHashMap;

use crate::draw::arena::{Arena, Span};
use crate::draw::batcher::Work;
use crate::draw::chunks::{Chunk, Segment};
use crate::draw::clips::{ClipPool, HeldClips};
use crate::draw::emit::Band;
use crate::draw::glyph_refs::GlyphRefs;
use crate::draw::groups::GroupTables;
use crate::draw::instance::GpuInstance;
use crate::draw::moves::Moves;
use crate::draw::occlusion::may_occlude;
use crate::draw::shown::VisibilityTable;
use crate::draw::zooms::Zooms;
use crate::scene::{Audience, Changes, Node, NodeId, Scene, SceneId, Viewer};
use crate::text::atlas::{AtlasKey, GlyphAtlas};
use rikka_gpu::reclaim::Reclaimer;
use rikka_gpu::texture::TextureId;

pub(crate) const UNPLACED: u32 = u32::MAX;

const MEMBERS_IN_DETAIL: u32 = 64;

pub(crate) enum Reclaimed {
    Glyphs(Vec<AtlasKey>),
    Instances(SceneId),
}

#[derive(Debug, Default)]
pub(crate) struct SceneCache {
    pub(crate) changes: Changes,
    pub(crate) retained: Option<RetainedBatch>,
    pub(crate) direct: Direct,
    pub(crate) prepared: Option<PreparedBatch>,
    pub(crate) zooms: Zooms,
}

#[derive(Debug)]
pub(crate) struct PreparedBatch {
    pub(crate) owner: Reclaimer,
    pub(crate) work: Work,
    pub(crate) pinned: Vec<AtlasKey>,
    pub(crate) book_revision: u64,
}

impl PreparedBatch {
    pub(crate) fn settle(mut self, atlas: &mut GlyphAtlas) -> Work {
        atlas.release(&std::mem::take(&mut self.pinned));
        self.work
    }
}

impl Drop for PreparedBatch {
    fn drop(&mut self) {
        if !self.pinned.is_empty() {
            self.owner
                .route(Reclaimed::Glyphs(std::mem::take(&mut self.pinned)));
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Direct {
    pub(crate) arena: Arena,
    pub(crate) chunks: Vec<Chunk>,
    pub(crate) segments: Vec<Segment>,
    pub(crate) groups: GroupTables,
    pub(crate) shown: VisibilityTable,
    pub(crate) captured: VisibilityTable,
    pub(crate) moves: Moves,
    pub(crate) extras: ClipPool,
    pub(crate) textured: Vec<u32>,
}

impl Direct {
    pub(crate) fn restrict(&mut self, id: NodeId, node: &Node, instances: &mut [GpuInstance]) {
        if node.audience == Audience::Everyone {
            return;
        }
        let place = VisibilityTable::place(id.index);
        self.shown.set(place, node.seen_by(Viewer::Screen));
        self.captured.set(place, node.seen_by(Viewer::Capture));
        self.moves.set(place, [0.0; 2]);
        for instance in instances {
            instance.shown = place;
        }
    }
}

#[derive(Debug)]
pub(crate) struct RetainedBatch {
    owner: Reclaimer,
    scene: SceneId,
    pub(crate) slots: Vec<Slot>,
    pub(crate) arena: Arena,
    pub(crate) chunks: Vec<Chunk>,
    pub(crate) segments: Vec<Segment>,
    pub(crate) placed: Vec<NodeId>,
    pub(crate) groups: GroupTables,
    pub(crate) shown: VisibilityTable,
    pub(crate) captured: VisibilityTable,
    pub(crate) moves: Moves,
    pub(crate) extras: ClipPool,
    pub(crate) textured: Vec<u32>,
    pub(crate) listings: FxHashMap<TextureId, Option<(u32, [u32; 2])>>,
    pub(crate) book_revision: Option<u64>,
    pub(crate) built: bool,
    pub(crate) chunk_at: Vec<u32>,
    pub(crate) next_chunk: u32,
    pub(crate) occluders: Vec<NodeId>,
}

#[derive(Clone, Debug)]
pub(crate) struct Slot {
    pub(crate) generation: u32,
    pub(crate) live: bool,
    pub(crate) queued: bool,
    pub(crate) span: Span,
    pub(crate) band: Band,
    pub(crate) ahead: Band,
    pub(crate) glyphs: GlyphRefs,
    pub(crate) chunk: u32,
    pub(crate) is_glass: bool,
    pub(crate) has_backdrop_segment: bool,
    pub(crate) occludes: bool,
    pub(crate) at: Point,
    pub(crate) clips: HeldClips,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            generation: 0,
            live: false,
            queued: false,
            span: Span::default(),
            band: Band::default(),
            ahead: Band::default(),
            glyphs: GlyphRefs::default(),
            chunk: UNPLACED,
            is_glass: false,
            has_backdrop_segment: false,
            occludes: false,
            at: Point::ZERO,
            clips: HeldClips::default(),
        }
    }
}

impl SceneCache {
    pub(crate) fn split(
        &mut self,
        owner: &Reclaimer,
        scene: SceneId,
    ) -> (&mut RetainedBatch, &Changes) {
        let fits = self
            .retained
            .as_ref()
            .is_some_and(|retained| retained.owner.same(owner));
        if !fits {
            if let Some(stale) = self.retained.take() {
                stale.release();
            }
            self.retained = Some(RetainedBatch::new(owner.clone(), scene));
        }
        let retained = self.retained.as_mut().expect("retained batch exists");
        (retained, &self.changes)
    }

    pub(crate) fn prepared_by(&self, owner: &Reclaimer, moved: u64) -> bool {
        self.prepared
            .as_ref()
            .is_some_and(|prepared| prepared.owner.same(owner) && prepared.book_revision >= moved)
    }

    pub(crate) fn forget(&mut self, atlas: &mut GlyphAtlas) {
        if let Some(prepared) = self.prepared.take() {
            prepared.settle(atlas);
        }
        if let Some(mut retained) = self.retained.take() {
            let mut keys = Vec::new();
            for slot in &retained.slots {
                keys.extend(slot.glyphs.keys());
            }
            if !keys.is_empty() {
                atlas.release(&keys);
            }
            retained.slots.clear();
        }
    }
}

impl Drop for SceneCache {
    fn drop(&mut self) {
        if let Some(retained) = self.retained.take() {
            retained.release();
        }
    }
}

impl RetainedBatch {
    fn new(owner: Reclaimer, scene: SceneId) -> Self {
        Self {
            owner,
            scene,
            slots: Vec::new(),
            arena: Arena::default(),
            chunks: Vec::new(),
            segments: Vec::new(),
            placed: Vec::new(),
            groups: GroupTables::default(),
            shown: VisibilityTable::default(),
            captured: VisibilityTable::default(),
            moves: Moves::default(),
            extras: ClipPool::default(),
            textured: Vec::new(),
            listings: FxHashMap::default(),
            book_revision: None,
            built: false,
            chunk_at: Vec::new(),
            next_chunk: 0,
            occluders: Vec::new(),
        }
    }

    fn release(mut self) {
        let mut keys = Vec::new();
        for slot in self.slots.drain(..) {
            keys.extend(slot.glyphs.keys());
        }
        if !keys.is_empty() {
            self.owner.route(Reclaimed::Glyphs(keys));
        }
        self.owner.route(Reclaimed::Instances(self.scene));
    }

    pub(crate) fn slot(&mut self, id: NodeId) -> &mut Slot {
        let index = id.index as usize;
        if self.slots.len() <= index {
            self.slots.resize_with(index + 1, Slot::default);
        }
        let slot = &mut self.slots[index];
        if !slot.live || slot.generation != id.generation {
            *slot = Slot {
                generation: id.generation,
                live: true,
                ..Slot::default()
            };
        }
        slot
    }

    pub(crate) fn prefetch(&self, id: NodeId) {
        if let Some(slot) = self.slots.get(id.index as usize) {
            rikka_simd::prefetch(slot, size_of::<Slot>());
        }
    }

    pub(crate) fn tracked_slot(&mut self, id: NodeId) -> Option<&mut Slot> {
        self.slots
            .get_mut(id.index as usize)
            .filter(|slot| slot.live && slot.generation == id.generation)
    }

    pub(crate) fn holds(&self, id: NodeId) -> Option<&Slot> {
        self.slots
            .get(id.index as usize)
            .filter(|slot| slot.live && slot.generation == id.generation)
    }

    pub(crate) fn show(&mut self, place: u32, node: &Node) {
        self.shown.set(place, node.seen_by(Viewer::Screen));
        self.captured.set(place, node.seen_by(Viewer::Capture));
    }

    pub(crate) fn hide(&mut self, place: u32) {
        self.shown.set(place, false);
        self.captured.set(place, false);
    }

    pub(crate) fn track_occluder(&mut self, id: NodeId, node: &Node) {
        let occludes = may_occlude(node);
        let slot = self.slot(id);
        if slot.occludes == occludes {
            return;
        }
        slot.occludes = occludes;
        match occludes {
            true => self.occluders.push(id),
            false => self.remove_occluder(id),
        }
    }

    pub(crate) fn untrack_occluder(&mut self, id: NodeId) {
        if let Some(slot) = self.tracked_slot(id)
            && std::mem::take(&mut slot.occludes)
        {
            self.remove_occluder(id);
        }
    }

    fn remove_occluder(&mut self, id: NodeId) {
        if let Some(pos) = self.occluders.iter().position(|occluder| *occluder == id) {
            self.occluders.swap_remove(pos);
        }
    }

    pub(crate) fn reindex(&mut self) {
        self.chunk_at.clear();
        let reach = self
            .chunks
            .iter()
            .map(|chunk| chunk.id + 1)
            .max()
            .unwrap_or(0);
        self.chunk_at.resize(reach as usize, UNPLACED);
        for (at, chunk) in self.chunks.iter().enumerate() {
            self.chunk_at[chunk.id as usize] = at as u32;
        }
        self.next_chunk = self.next_chunk.max(reach);
    }

    pub(crate) fn chunk_index(&self, id: u32) -> Option<usize> {
        self.chunk_at
            .get(id as usize)
            .filter(|at| **at != UNPLACED)
            .map(|at| *at as usize)
    }

    pub(crate) fn painted_above(
        &self,
        scene: &Scene,
        backdrop: NodeId,
        into: &mut Vec<Rect>,
    ) -> bool {
        let Some(at) = self.segments.iter().position(
            |segment| matches!(segment, Segment::Backdrop { node, .. } if *node == backdrop),
        ) else {
            return false;
        };
        for segment in &self.segments[at + 1..] {
            match *segment {
                Segment::Draw { first, count } => {
                    for chunk in &self.chunks[first as usize..(first + count) as usize] {
                        let (from, to) = chunk.members;
                        match to - from <= MEMBERS_IN_DETAIL {
                            true => into.extend(
                                self.placed[from as usize..to as usize]
                                    .iter()
                                    .filter(|id| {
                                        self.holds(**id).is_some_and(|slot| slot.chunk == chunk.id)
                                    })
                                    .filter_map(|id| scene.get(*id))
                                    .filter(|node| node.paints())
                                    .map(|node| scene.bounds_of(node))
                                    .filter(|bounds| !bounds.is_empty()),
                            ),
                            false if !chunk.bounds.is_empty() => into.push(chunk.bounds),
                            false => {}
                        }
                    }
                }
                Segment::Backdrop { node, .. } => {
                    if let Some(node) = scene.get(node).filter(|node| node.paints()) {
                        into.push(scene.bounds_of(node));
                    }
                }
                Segment::Capture { region } => into.push(region),
                Segment::Fade { .. } => {}
            }
        }
        true
    }

    pub(crate) fn chunk_summary(&self, scene: &Scene, chunk: usize) -> (Rect, bool) {
        let entry = self.chunks[chunk];
        let (from, to) = entry.members;
        let mut bounds = Rect::ZERO;
        let mut has_textured = false;
        for &id in &self.placed[from as usize..to as usize] {
            if !self.holds(id).is_some_and(|slot| slot.chunk == entry.id) {
                continue;
            }
            let Some(node) = scene.get(id).filter(|node| node.paints()) else {
                continue;
            };
            has_textured |= node.is_textured();
            let painted = scene.bounds_of(node);
            if !painted.is_empty() {
                bounds = match bounds.is_empty() {
                    true => painted,
                    false => bounds.union(&painted),
                };
            }
        }
        (bounds, has_textured)
    }
}
