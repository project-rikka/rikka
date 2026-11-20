mod glyphs;
mod output;
mod prepare_retained;
mod sources;

use crate::draw::arena::Span;
use crate::draw::chunks::{Run, Segment};
use crate::draw::emit::{emit_node, glass_instance, shown_band};
use crate::draw::fades::Fades;
use crate::draw::instance::GpuInstance;
use crate::draw::order_positions::OrderPositions;
use crate::draw::retained::{PreparedBatch, SceneCache};
use crate::draw::zooms::Zooms;
use crate::scene::{NodeId, Rank, Scene};
use crate::text::atlas::{AtlasKey, GlyphRef};
use crate::text::glyph_holder::GlyphHolder;
use crate::text::routing::GlyphRoute;
use rikka_gpu::error::Result;

use prepare_retained::{Reach, clips, is_glass, offsets, textured};

pub(crate) use output::{Batch, Ledgered, Upload, Work};
pub(crate) use sources::Sources;

#[derive(Debug, Default)]
pub(crate) struct Batcher {
    keys: Vec<GlyphRoute>,
    glyphs: Vec<Option<GlyphRef>>,
    glyph_holder: GlyphHolder,
    ahead: Vec<AtlasKey>,
    reach: Vec<(NodeId, Reach)>,
    queue: Vec<NodeId>,
    ranked: Vec<(Option<Rank>, NodeId)>,
    stale: Vec<u32>,
    dropped: Vec<AtlasKey>,
    arrivals: Vec<NodeId>,
    positions: OrderPositions,
    reframed: Vec<NodeId>,
    vacated: Vec<Span>,
    rows: Vec<(f32, f32, bool)>,
    scratch: Vec<GpuInstance>,
    fades: Fades,
}

impl Batcher {
    pub(crate) fn prepare(&mut self, scene: &mut Scene, sources: &mut Sources<'_>) -> Result<()> {
        let book_revision = sources.book.revision();
        let mut cache = scene.cache.take().unwrap_or_default();
        if let Some(earlier) = cache.prepared.take() {
            earlier.settle(&mut sources.atlas.lock());
        }
        scene.take_changes(&mut cache.changes);
        let mut zooms = std::mem::take(&mut cache.zooms);
        match cache.changes.reset {
            true => zooms.clear(),
            false => zooms.forget(&cache.changes.removed),
        }
        let prepared = match scene.is_immediate() {
            true => self.prepare_direct(scene, &mut cache, &mut zooms, sources),
            false => self.prepare_retained(scene, &mut cache, &mut zooms, sources),
        };
        cache.zooms = zooms;
        let outcome = match prepared {
            Ok((work, pinned)) => {
                cache.prepared = Some(PreparedBatch {
                    owner: sources.owner.clone(),
                    work,
                    pinned,
                    book_revision,
                });
                Ok(())
            }
            Err(error) => {
                cache.forget(&mut sources.atlas.lock());
                if !scene.is_immediate() {
                    scene.damage_all();
                }
                Err(error)
            }
        };
        scene.cache = Some(cache);
        outcome
    }

    pub(crate) fn batch(scene: &Scene) -> Option<Batch<'_>> {
        let cache = scene.cache.as_ref()?;
        Some(match cache.retained.as_ref() {
            Some(retained) if !scene.is_immediate() => Batch {
                chunks: &retained.chunks,
                segments: &retained.segments,
                textured: &retained.textured,
                retained: Some(retained),
                upload: Upload {
                    scene: Some(scene.id()),
                    instances: Ledgered {
                        data: &retained.arena.data,
                        ledger: &retained.arena.ledger,
                    },
                    groups: offsets(&retained.groups),
                    clips: clips(&retained.groups),
                    shown: Ledgered {
                        data: &retained.shown.words,
                        ledger: &retained.shown.ledger,
                    },
                    captured: Ledgered {
                        data: &retained.captured.words,
                        ledger: &retained.captured.ledger,
                    },
                    moves: Ledgered {
                        data: &retained.moves.offsets,
                        ledger: &retained.moves.ledger,
                    },
                    extras: Ledgered {
                        data: &retained.extras.entries,
                        ledger: &retained.extras.ledger,
                    },
                },
            },
            _ => {
                let direct = &cache.direct;
                Batch {
                    chunks: &direct.chunks,
                    segments: &direct.segments,
                    textured: &direct.textured,
                    retained: None,
                    upload: Upload {
                        scene: None,
                        instances: Ledgered {
                            data: &direct.arena.data,
                            ledger: &direct.arena.ledger,
                        },
                        groups: offsets(&direct.groups),
                        clips: clips(&direct.groups),
                        shown: Ledgered {
                            data: &direct.shown.words,
                            ledger: &direct.shown.ledger,
                        },
                        captured: Ledgered {
                            data: &direct.captured.words,
                            ledger: &direct.captured.ledger,
                        },
                        moves: Ledgered {
                            data: &direct.moves.offsets,
                            ledger: &direct.moves.ledger,
                        },
                        extras: Ledgered {
                            data: &direct.extras.entries,
                            ledger: &direct.extras.ledger,
                        },
                    },
                }
            }
        })
    }

    fn prepare_direct(
        &mut self,
        scene: &mut Scene,
        cache: &mut SceneCache,
        zooms: &mut Zooms,
        sources: &mut Sources<'_>,
    ) -> Result<(Work, Vec<AtlasKey>)> {
        for &id in zooms.tick() {
            scene.damage_node(id);
        }
        self.keys.clear();
        self.reach.clear();
        self.ahead.clear();
        for &id in scene.settled_order() {
            let Some(node) = scene.get(id).filter(|node| node.paints()) else {
                continue;
            };
            let from = self.keys.len();
            self.collect_routes(zooms, id, node, shown_band(scene, node), sources);
            self.reach.push((id, Reach::Fresh(from, self.keys.len())));
        }
        self.draw_ahead(sources);
        self.hold_glyphs(sources)?;
        let pinned: Vec<AtlasKey> = self.held_keys().collect();

        let Self {
            scratch,
            fades,
            glyphs,
            reach,
            ..
        } = self;
        let direct = &mut cache.direct;
        fades.reset();
        direct.arena.relayout();
        direct.extras.reset();
        direct.chunks.clear();
        direct.segments.clear();
        direct.textured.clear();
        let mut reach = reach.iter().peekable();
        let mut run = Run::default();
        let mut walked = 0;
        for (member, &id) in scene.settled_order().iter().enumerate() {
            let Some(node) = scene.get(id) else { continue };
            walked += 1;
            if !node.paints() {
                continue;
            }
            let refs = match reach.next_if(|(at, _)| *at == id) {
                Some((_, Reach::Fresh(from, to))) => &glyphs[*from..*to],
                _ => &[][..],
            };
            let bounds = scene.bounds_of(node);
            if !fades.enter(
                scene,
                node,
                bounds,
                &mut run,
                &mut direct.chunks,
                &mut direct.segments,
            ) {
                continue;
            }
            let clips = direct.extras.push(node);
            scratch.clear();
            if is_glass(node) {
                run.flush(&mut direct.chunks, &mut direct.segments);
                scratch.push(glass_instance(node, clips));
                direct.restrict(id, node, scratch);
                let span = direct.arena.append(scratch);
                direct.segments.push(Segment::Backdrop {
                    node: id,
                    instance: span.first,
                    region: node.rect.translate(scene.offset_of(node.group)),
                });
                continue;
            }
            emit_node(
                scratch,
                node,
                sources.book,
                refs,
                shown_band(scene, node),
                clips,
            );
            direct.restrict(id, node, scratch);
            let before_textured = direct.textured.len();
            direct.textured.extend(textured(scratch));
            let has_textured = direct.textured.len() > before_textured;
            let span = direct.arena.append(scratch);
            run.take(
                &mut direct.chunks,
                span,
                bounds,
                has_textured,
                member as u32,
            );
        }
        fades.close_all(&mut run, &mut direct.chunks, &mut direct.segments);
        run.flush(&mut direct.chunks, &mut direct.segments);
        fades.settle_bounds(scene);
        direct.groups.lay_out(scene);
        let work = Work {
            walked,
            emitted: walked,
            reused: 0,
            restructured: true,
        };
        Ok((work, pinned))
    }
}
