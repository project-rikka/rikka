use rikka_sync::Mutex;
use rikka_text::GlyphRaster;

use crate::text::atlas::GlyphAtlas;
use crate::text::routing::GlyphRouting;
use crate::text::workers::GlyphWorkers;
use rikka_gpu::reclaim::Reclaimer;
use rikka_gpu::texture::TextureBook;

pub(crate) struct Sources<'a> {
    pub raster: &'a mut GlyphRaster,
    pub book: &'a TextureBook,
    pub atlas: &'a Mutex<GlyphAtlas>,
    pub workers: &'a GlyphWorkers,
    pub owner: &'a Reclaimer,
    pub completed: u64,
    pub routing: GlyphRouting,
}
