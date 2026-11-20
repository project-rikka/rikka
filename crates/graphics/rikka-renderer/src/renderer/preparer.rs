use std::sync::Arc;

use rikka_text::GlyphRaster;

use crate::draw::batcher::{Batcher, Sources};
use crate::renderer::Renderer;
use crate::scene::Scene;
use crate::text::atlas::SharedAtlas;
use crate::text::routing::GlyphRouting;
use crate::text::workers::GlyphWorkers;
use rikka_gpu::core::frame::FrameTimeline;
use rikka_gpu::error::Result;
use rikka_gpu::reclaim::Reclaimer;
use rikka_gpu::texture::TextureBook;

pub struct Preparer {
    raster: GlyphRaster,
    batcher: Batcher,
    shared: Shared,
}

#[derive(Clone)]
pub(crate) struct Shared {
    pub book: Arc<TextureBook>,
    pub atlas: SharedAtlas,
    pub workers: Arc<GlyphWorkers>,
    pub owner: Reclaimer,
    pub timeline: Arc<FrameTimeline>,
    pub routing: GlyphRouting,
}

impl Preparer {
    pub(crate) fn new(shared: Shared) -> Self {
        Self {
            raster: GlyphRaster::new(),
            batcher: Batcher::default(),
            shared,
        }
    }

    pub fn prepare(&mut self, scene: &mut Scene) -> Result<()> {
        scene.settle_order();
        let moved = self.shared.book.moved();
        let ready = scene
            .cache
            .as_ref()
            .is_some_and(|cache| cache.prepared_by(&self.shared.owner, moved));
        if ready && !scene.has_edits() {
            return Ok(());
        }
        let Shared {
            book,
            atlas,
            workers,
            owner,
            timeline,
            routing,
        } = &self.shared;
        self.batcher.prepare(
            scene,
            &mut Sources {
                raster: &mut self.raster,
                book,
                atlas,
                workers,
                owner,
                completed: timeline.finished()?,
                routing: *routing,
            },
        )
    }
}

impl Renderer {
    #[must_use]
    pub fn preparer(&self) -> Preparer {
        Preparer::new(self.painter.shared())
    }
}

impl std::fmt::Debug for Preparer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Preparer").finish_non_exhaustive()
    }
}
