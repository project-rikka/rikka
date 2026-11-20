use rikka_sync::Mutex;

use crate::draw::emit::{Band, collect_keys};
use crate::draw::zooms::{Phase, Zooms};
use crate::scene::{Node, NodeId, Shape};
use crate::text::atlas::{AtlasKey, GlyphAtlas, GlyphRef};
use crate::text::glyph_holder::GlyphSources;
use crate::text::routing::GlyphRoute;
use rikka_gpu::error::Result;

use super::Batcher;
use super::sources::Sources;

fn lacking(atlas: &Mutex<GlyphAtlas>, routes: &[GlyphRoute], ahead: &mut Vec<AtlasKey>) -> bool {
    let atlas = atlas.lock();
    let before = ahead.len();
    ahead.extend(
        routes
            .iter()
            .map(|route| route.key)
            .filter(|key| atlas.lacks(key)),
    );
    ahead.len() > before
}

impl Batcher {
    pub(super) fn hold_glyphs(&mut self, sources: &mut Sources<'_>) -> Result<()> {
        self.glyph_holder.acquire_glyphs(
            GlyphSources {
                atlas: sources.atlas,
                workers: sources.workers,
                raster: sources.raster,
                completed: sources.completed,
            },
            &self.keys,
            &mut self.glyphs,
        )
    }

    pub(super) fn collect_routes(
        &mut self,
        zooms: &mut Zooms,
        id: NodeId,
        node: &Node,
        band: Band,
        sources: &Sources<'_>,
    ) -> Phase {
        let phase = match &node.shape {
            Shape::Text(paint) => zooms.phase(id, paint.scale),
            _ => Phase::Settled,
        };
        let zooming = match phase {
            Phase::Moving => true,
            Phase::Settled => false,
            Phase::Converting { patient } => {
                let from = self.keys.len();
                collect_keys(&mut self.keys, node, band, sources.routing, false);
                if !patient || !lacking(sources.atlas, &self.keys[from..], &mut self.ahead) {
                    zooms.converted(id);
                    return Phase::Settled;
                }
                self.keys.truncate(from);
                true
            }
        };
        collect_keys(&mut self.keys, node, band, sources.routing, zooming);
        phase
    }

    pub(super) fn draw_ahead(&mut self, sources: &Sources<'_>) {
        if self.ahead.is_empty() || !sources.workers.available() {
            self.ahead.clear();
            return;
        }
        {
            let mut atlas = sources.atlas.lock();
            self.ahead.retain(|key| atlas.begin_warming(*key));
        }
        sources.workers.draw_ahead(self.ahead.drain(..));
    }

    pub(super) fn held_keys(&self) -> impl Iterator<Item = AtlasKey> + '_ {
        self.glyphs
            .iter()
            .flatten()
            .filter_map(GlyphRef::placed_key)
    }
}
