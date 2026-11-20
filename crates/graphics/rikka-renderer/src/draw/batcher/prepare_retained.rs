use std::sync::Arc;

use rikka_types::Point;

use crate::draw::arena::Span;
use crate::draw::chunks::{Run, Segment, too_scattered};
use crate::draw::emit::{
    Band, ahead_band, collect_ahead_keys, emit_node, glass_instance, resident_band, shown_band,
};
use crate::draw::fades::{reframe, update_fades};
use crate::draw::groups::GroupTables;
use crate::draw::instance::{GpuGroupClip, GpuInstance, KIND_TEXTURED};
use crate::draw::retained::{RetainedBatch, SceneCache, UNPLACED};
use crate::draw::shown::VisibilityTable;
use crate::draw::zooms::{Phase, Zooms};
use crate::scene::{
    BOUNDS, CONTENT, Dirt, LAYER, MOVED, NEW, Node, NodeId, SHIFTED, SHOWN, Scene, Shape,
};
use crate::text::atlas::AtlasKey;
use rikka_gpu::error::Result;
use rikka_gpu::texture::TextureBook;

use super::Batcher;
use super::output::{Ledgered, Work};
use super::sources::Sources;

#[derive(Clone, Copy, Debug)]
pub(super) enum Reach {
    Fresh(usize, usize),
    Unchanged,
}

#[derive(Default)]
struct Emission {
    emitted: usize,
    restructure: bool,
}

#[derive(Default)]
struct Applied {
    walked: usize,
    restructure: bool,
}

fn pack(scene: &Scene, retained: &mut RetainedBatch) {
    let RetainedBatch { slots, arena, .. } = retained;
    let mut order: Vec<Span> = Vec::new();
    let mut at: Vec<usize> = Vec::new();
    for id in scene.settled_order() {
        let index = id.index as usize;
        if let Some(slot) = slots
            .get(index)
            .filter(|slot| slot.live && slot.generation == id.generation && slot.span.room > 0)
        {
            order.push(slot.span);
            at.push(index);
        }
    }
    arena.compact(order.iter_mut());
    for (span, index) in order.into_iter().zip(at) {
        slots[index].span = span;
    }
}

pub(super) fn offsets(tables: &GroupTables) -> Ledgered<'_, [f32; 2]> {
    Ledgered {
        data: &tables.offsets,
        ledger: &tables.offset_ledger,
    }
}

pub(super) fn clips(tables: &GroupTables) -> Ledgered<'_, GpuGroupClip> {
    Ledgered {
        data: &tables.clips,
        ledger: &tables.clip_ledger,
    }
}

pub(super) fn textured(instances: &[GpuInstance]) -> impl Iterator<Item = u32> + '_ {
    instances
        .iter()
        .filter(|instance| instance.kind() == KIND_TEXTURED)
        .map(|instance| instance.tex)
}

pub(super) fn is_glass(node: &Node) -> bool {
    node.backdrop.is_some() && node.shape.fills_interior()
}

fn collect_textured(scene: &Scene, retained: &mut RetainedBatch) {
    retained.textured.clear();
    for id in scene.textured() {
        if !scene.get(id).is_some_and(Node::paints) {
            continue;
        }
        let Some(span) = retained
            .tracked_slot(id)
            .filter(|slot| slot.chunk != UNPLACED)
            .map(|slot| slot.span)
        else {
            continue;
        };
        let at = span.first as usize;
        let drawn = &retained.arena.data[at..at + span.count as usize];
        retained.textured.extend(textured(drawn));
    }
}

impl Batcher {
    pub(super) fn prepare_retained(
        &mut self,
        scene: &mut Scene,
        cache: &mut SceneCache,
        zooms: &mut Zooms,
        sources: &mut Sources<'_>,
    ) -> Result<(Work, Vec<AtlasKey>)> {
        if cache.changes.reset {
            cache.forget(&mut sources.atlas.lock());
        }
        let (retained, changes) = cache.split(sources.owner, scene.id());
        let fresh = !retained.built;
        let mut restructure = fresh || changes.resequenced || changes.restacked_layers;
        self.open_frame(retained);
        self.remove_nodes(retained, &changes.removed);
        if fresh {
            self.queue_all(scene, retained);
        }
        let applied = self.apply_changes(scene, retained, &changes.nodes);
        let mut walked = applied.walked;
        restructure |= applied.restructure;
        self.requeue_relisted(scene, retained, sources.book);
        self.queue_zoom_ticks(scene, retained, zooms);

        self.route_queued(scene, retained, zooms, sources);
        self.draw_ahead(sources);
        if let Err(error) = self.hold_glyphs(sources) {
            sources.atlas.lock().release(&self.dropped);
            return Err(error);
        }

        let emission = self.emit_queued(scene, retained, sources.book);
        restructure |= emission.restructure;
        if !self.dropped.is_empty() {
            sources.atlas.lock().release(&self.dropped);
        }

        if !restructure {
            restructure = !self.place_inserted(scene, retained);
        }
        match restructure {
            true => walked += self.relayout_retained(scene, retained),
            false => self.refresh_stale_chunks(scene, retained, changes.faded_layers),
        }
        let placed = retained.placed.len();
        if !scene.has_textures() {
            retained.textured.clear();
        } else if restructure
            || !self.queue.is_empty()
            || !changes.removed.is_empty()
            || changes.nodes.iter().any(|(_, dirt)| dirt & SHOWN != 0)
        {
            collect_textured(scene, retained);
        }
        retained.groups.refresh(
            scene,
            &changes.moved_groups,
            &changes.reclipped_groups,
            fresh || changes.reset,
        );
        retained.built = true;
        let work = Work {
            walked,
            emitted: emission.emitted,
            reused: placed.saturating_sub(emission.emitted),
            restructured: restructure,
        };
        Ok((work, Vec::new()))
    }

    fn open_frame(&mut self, retained: &mut RetainedBatch) {
        retained.arena.ledger.open(retained.arena.data.len());
        retained.groups.open();
        retained.shown.ledger.open(retained.shown.words.len());
        retained.captured.ledger.open(retained.captured.words.len());
        retained.moves.ledger.open(retained.moves.offsets.len());
        retained.extras.open();
        self.queue.clear();
        self.stale.clear();
        self.dropped.clear();
        self.arrivals.clear();
        self.reframed.clear();
        self.vacated.clear();
    }

    fn remove_nodes(&mut self, retained: &mut RetainedBatch, removed: &[NodeId]) {
        for id in removed {
            retained.untrack_occluder(*id);
            let Some(slot) = retained.tracked_slot(*id) else {
                continue;
            };
            let (span, chunk) = (slot.span, slot.chunk);
            let mut held = std::mem::take(&mut slot.clips);
            slot.glyphs.release(&mut self.dropped);
            slot.live = false;
            retained.extras.release(&mut held);
            let place = VisibilityTable::place(id.index);
            retained.arena.clear(span);
            retained.arena.release(span);
            retained.hide(place);
            retained.moves.set(place, [0.0; 2]);
            if chunk != UNPLACED {
                self.stale.push(chunk);
            }
        }
    }

    fn queue_all(&mut self, scene: &Scene, retained: &mut RetainedBatch) {
        for &id in scene.settled_order() {
            if let Some(node) = scene.get(id) {
                retained.track_occluder(id, node);
            }
            retained.slot(id).queued = true;
            self.queue.push(id);
        }
    }

    fn apply_changes(
        &mut self,
        scene: &Scene,
        retained: &mut RetainedBatch,
        nodes: &[(NodeId, Dirt)],
    ) -> Applied {
        let mut applied = Applied::default();
        for &(id, dirt) in nodes {
            let Some(node) = scene.get(id) else { continue };
            applied.walked += 1;
            retained.track_occluder(id, node);
            let mut emit = dirt & (NEW | CONTENT) != 0;
            if emit {
                retained.slot(id).ahead = Band::default();
            }
            applied.restructure |= self.note_segment_change(retained, id, node, dirt);
            if dirt & SHOWN != 0 {
                applied.restructure |= self.note_visibility_change(retained, id, node);
            }
            if dirt & (BOUNDS | SHIFTED) != 0 && self.note_bounds_change(scene, retained, id, node)
            {
                emit = true;
            }
            if dirt & MOVED != 0 && !emit && !retained.relocate(id, node) {
                emit = true;
            }
            let slot = retained.slot(id);
            if emit && !slot.queued {
                slot.queued = true;
                self.queue.push(id);
            }
        }
        applied
    }

    fn note_segment_change(
        &mut self,
        retained: &mut RetainedBatch,
        id: NodeId,
        node: &Node,
        dirt: Dirt,
    ) -> bool {
        let slot = retained.slot(id);
        let was_glass = slot.is_glass;
        let has_segment = slot.has_backdrop_segment;
        if node.layer.is_none() && node.backdrop.is_none() && !was_glass {
            return false;
        }
        if dirt & (NEW | CONTENT | BOUNDS | SHIFTED | MOVED | SHOWN) != 0 {
            self.reframed.push(id);
        }
        dirt & LAYER != 0 || has_segment != (is_glass(node) && node.paints())
    }

    fn note_visibility_change(
        &mut self,
        retained: &mut RetainedBatch,
        id: NodeId,
        node: &Node,
    ) -> bool {
        let slot = retained.slot(id);
        let restructure = match slot.chunk {
            UNPLACED => slot.span.room > 0,
            chunk => {
                self.stale.push(chunk);
                false
            }
        };
        retained.show(VisibilityTable::place(id.index), node);
        restructure
    }

    fn note_bounds_change(
        &mut self,
        scene: &Scene,
        retained: &mut RetainedBatch,
        id: NodeId,
        node: &Node,
    ) -> bool {
        let slot = retained.slot(id);
        if slot.chunk != UNPLACED {
            self.stale.push(slot.chunk);
        }
        matches!(node.shape, Shape::Text(_)) && !slot.band.covers(shown_band(scene, node))
    }

    fn queue_zoom_ticks(
        &mut self,
        scene: &mut Scene,
        retained: &mut RetainedBatch,
        zooms: &mut Zooms,
    ) {
        for &id in zooms.tick() {
            if let Some(slot) = retained.tracked_slot(id)
                && !slot.queued
            {
                slot.queued = true;
                self.queue.push(id);
            }
            scene.damage_node(id);
        }
    }

    fn sort_queue(&mut self, scene: &Scene) {
        self.ranked.clear();
        self.ranked.extend(
            self.queue
                .iter()
                .map(|id| (scene.get(*id).map(Node::rank), *id)),
        );
        self.ranked.sort_unstable_by_key(|(rank, _)| *rank);
        self.queue.clear();
        self.queue.extend(self.ranked.iter().map(|(_, id)| *id));
    }

    fn route_queued(
        &mut self,
        scene: &Scene,
        retained: &mut RetainedBatch,
        zooms: &mut Zooms,
        sources: &Sources<'_>,
    ) {
        self.keys.clear();
        self.reach.clear();
        self.ahead.clear();
        self.sort_queue(scene);
        let queue = std::mem::take(&mut self.queue);
        for (at, &id) in queue.iter().enumerate() {
            prefetch_ahead(scene, retained, &queue[at + 1..]);
            let Some(node) = scene.get(id).filter(|node| node.draws()) else {
                continue;
            };
            let shown = shown_band(scene, node);
            let band = match &node.shape {
                Shape::Text(paint) if zooms.is_settled(id, paint.scale) => resident_band(
                    scene,
                    node,
                    shown,
                    sources.routing,
                    &mut self.rows,
                    sources.atlas,
                ),
                _ => shown,
            };
            retained.slot(id).band = band;
            let from = self.keys.len();
            let phase = self.collect_routes(zooms, id, node, band, sources);
            let ahead = match phase {
                Phase::Settled => ahead_band(scene, node),
                _ => Band::default(),
            };
            let prefetched = std::mem::replace(&mut retained.slot(id).ahead, ahead);
            collect_ahead_keys(
                &mut self.ahead,
                node,
                [band, prefetched],
                ahead,
                sources.routing,
            );
            let to = self.keys.len();
            let reach = match retained.slot(id).glyphs.matches(&self.keys[from..to]) {
                true => {
                    self.keys.truncate(from);
                    Reach::Unchanged
                }
                false => Reach::Fresh(from, to),
            };
            self.reach.push((id, reach));
        }
        self.queue = queue;
    }

    fn emit_queued(
        &mut self,
        scene: &Scene,
        retained: &mut RetainedBatch,
        book: &TextureBook,
    ) -> Emission {
        let mut reach = self.reach.iter().peekable();
        let mut emission = Emission::default();
        for (at, &id) in self.queue.iter().enumerate() {
            prefetch_ahead(scene, retained, &self.queue[at + 1..]);
            let Some(node) = scene.get(id) else { continue };
            let fresh = match reach.next_if(|(at, _)| *at == id) {
                Some((_, Reach::Fresh(from, to))) => Some((*from, *to)),
                Some((_, Reach::Unchanged)) => None,
                None => Some((0, 0)),
            };
            let slot = retained.slot(id);
            slot.queued = false;
            if let Some((from, to)) = fresh {
                slot.glyphs.retain(
                    &self.keys[from..to],
                    &self.glyphs[from..to],
                    &mut self.dropped,
                );
            }
            let (span, chunk, band) = (slot.span, slot.chunk, slot.band);
            let mut held = slot.clips;
            let clips = retained.extras.hold(&mut held, node);
            retained.slot(id).clips = held;
            let place = VisibilityTable::place(id.index);
            retained.show(place, node);
            self.scratch.clear();
            if node.draws() {
                match is_glass(node) {
                    true => self.scratch.push(glass_instance(node, clips)),
                    false => emit_node(
                        &mut self.scratch,
                        node,
                        book,
                        retained.slot(id).glyphs.refs(),
                        band,
                        clips,
                    ),
                }
            }
            for instance in &mut self.scratch {
                instance.shown = place;
            }
            let written = match self.scratch.is_empty() {
                true if span.room == 0 => continue,
                true => {
                    retained.slot(id).span = retained.arena.clear(span);
                    if chunk != UNPLACED {
                        self.stale.push(chunk);
                    }
                    continue;
                }
                false => retained.arena.write(span, &self.scratch),
            };
            emission.emitted += 1;
            retained.slot(id).span = written.span;
            retained.slot(id).at = Point::new(node.rect.min_x(), node.rect.min_y());
            retained.moves.set(place, [0.0; 2]);
            if chunk != UNPLACED {
                self.stale.push(chunk);
            }
            if written.moved || chunk == UNPLACED {
                match !is_glass(node) {
                    true => {
                        if written.moved && span.room > 0 {
                            self.vacated.push(span);
                        }
                        retained.slot(id).chunk = UNPLACED;
                        self.arrivals.push(id);
                    }
                    false => emission.restructure = true,
                }
            }
        }
        emission
    }

    fn relayout_retained(&mut self, scene: &mut Scene, retained: &mut RetainedBatch) -> usize {
        let mut walked = self.lay_out(scene, retained);
        if too_scattered(
            retained.chunks.len(),
            retained.segments.len(),
            retained.arena.used,
        ) || retained.arena.is_fragmented()
        {
            pack(scene, retained);
            walked += self.lay_out(scene, retained);
        }
        walked
    }

    fn refresh_stale_chunks(
        &mut self,
        scene: &mut Scene,
        retained: &mut RetainedBatch,
        faded_layers: bool,
    ) {
        for chunk in &mut self.stale {
            *chunk = retained
                .chunk_index(*chunk)
                .map_or(UNPLACED, |at| at as u32);
        }
        self.stale.sort_unstable();
        self.stale.dedup();
        for &chunk in &self.stale {
            let at = chunk as usize;
            if at < retained.chunks.len() {
                let (bounds, has_textured) = retained.chunk_summary(scene, at);
                retained.chunks[at].bounds = bounds;
                retained.chunks[at].has_textured = has_textured;
            }
        }
        reframe(scene, &mut retained.segments, &self.reframed);
        if faded_layers {
            update_fades(scene, &mut retained.segments);
        }
    }

    fn place_inserted(&mut self, scene: &Scene, retained: &mut RetainedBatch) -> bool {
        for span in self.vacated.drain(..) {
            retained.arena.clear(span);
        }
        match scene.has_layers() && self.arrivals.len() > 1 {
            true => self.positions.index(scene.settled_order()),
            false => self.positions.clear(),
        }
        for &id in &self.arrivals {
            if !retained.place(scene, id, &self.positions) {
                return false;
            }
        }
        !too_scattered(
            retained.chunks.len(),
            retained.segments.len(),
            retained.arena.used,
        )
    }

    fn requeue_relisted(
        &mut self,
        scene: &Scene,
        retained: &mut RetainedBatch,
        book: &TextureBook,
    ) {
        let revision = book.revision();
        if retained.book_revision == Some(revision) {
            return;
        }
        retained.book_revision = Some(revision);
        for (texture, users) in scene.texture_uses() {
            let now = book.get(texture).map(|listing| listing.drawn_as());
            if retained.listings.insert(texture, now) == Some(now) {
                continue;
            }
            for &id in users {
                let Some(slot) = retained.tracked_slot(id) else {
                    continue;
                };
                if !slot.queued {
                    slot.queued = true;
                    self.queue.push(id);
                }
            }
        }
        retained
            .listings
            .retain(|texture, _| scene.uses_texture(*texture));
    }

    fn lay_out(&mut self, scene: &mut Scene, retained: &mut RetainedBatch) -> usize {
        let fades = &mut self.fades;
        fades.reset();
        retained.chunks.clear();
        retained.segments.clear();
        retained.placed.clear();
        let mut run = Run::default();
        let mut walked = 0;
        for &id in scene.settled_order() {
            let Some(node) = scene.get(id) else { continue };
            walked += 1;
            let paints = node.paints();
            let Some(slot) = retained.tracked_slot(id) else {
                continue;
            };
            slot.chunk = UNPLACED;
            slot.is_glass = is_glass(node);
            slot.has_backdrop_segment = false;
            let span = slot.span;
            if span.room == 0 || (!paints && node.layer.is_some()) {
                continue;
            }
            let bounds = match paints {
                true => scene.bounds_of(node),
                false => rikka_types::Rect::ZERO,
            };
            if !fades.enter(
                scene,
                node,
                bounds,
                &mut run,
                &mut retained.chunks,
                &mut retained.segments,
            ) {
                continue;
            }
            if is_glass(node) && paints {
                run.flush(&mut retained.chunks, &mut retained.segments);
                retained.segments.push(Segment::Backdrop {
                    node: id,
                    instance: span.first,
                    region: node.rect.translate(scene.offset_of(node.group)),
                });
                if let Some(slot) = retained.tracked_slot(id) {
                    slot.has_backdrop_segment = true;
                }
                continue;
            }
            let has_textured = paints && node.is_textured();
            let member = retained.placed.len() as u32;
            run.take(&mut retained.chunks, span, bounds, has_textured, member);
            retained.placed.push(id);
            let open = retained.chunks.len() as u32;
            if let Some(slot) = retained.tracked_slot(id) {
                slot.chunk = open;
            }
        }
        fades.close_all(&mut run, &mut retained.chunks, &mut retained.segments);
        run.flush(&mut retained.chunks, &mut retained.segments);
        fades.settle_bounds(scene);
        retained.next_chunk = 0;
        retained.reindex();
        walked
    }
}

fn prefetch_ahead(scene: &Scene, retained: &RetainedBatch, ahead: &[NodeId]) {
    if let Some(&next) = ahead.first()
        && let Some(Shape::Text(paint)) = scene.get(next).map(|node| &node.shape)
    {
        rikka_simd::prefetch(
            Arc::as_ptr(&paint.layout),
            size_of::<rikka_text::TextLayout>(),
        );
    }
    if let Some(&further) = ahead.get(1) {
        scene.prefetch(further);
        retained.prefetch(further);
    }
}
