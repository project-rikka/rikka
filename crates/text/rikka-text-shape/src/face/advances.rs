use std::sync::OnceLock;

use rikka_text_font::{F2Dot14, Font, GlyphId, LongMetrics, MetricsVariations, RegionScalars};

use crate::buffer::glyph_info::GlyphInfo;
use crate::buffer::glyph_position::GlyphPosition;
use crate::face::atomic_cache::AtomicCache;

const CACHED_ADVANCES: usize = 2048;

#[derive(Debug)]
pub(crate) struct Advances {
    font: Font<'static>,
    hmtx: Option<LongMetrics<'static>>,
    cache: Option<AtomicCache<CACHED_ADVANCES>>,
    deltas: OnceLock<AdvanceDeltas>,
    half_em: i32,
}

#[derive(Debug)]
enum AdvanceDeltas {
    Static,
    Metrics(MetricsVariations<'static>, Box<RegionScalars>),
    Phantom,
}

impl Advances {
    pub(crate) fn new(font: Font<'static>, varied: bool) -> Self {
        Self {
            hmtx: font.hmtx(),
            cache: varied.then(AtomicCache::default),
            deltas: OnceLock::new(),
            half_em: i32::from(font.units_per_em()) / 2,
            font,
        }
    }

    #[inline]
    pub(crate) fn advance(&self, glyph: u32, coords: &[F2Dot14]) -> i32 {
        let Some(hmtx) = self.hmtx else {
            return if glyph < u32::from(self.font.glyph_count()) {
                self.half_em
            } else {
                0
            };
        };
        let id = GlyphId(glyph as u16);
        let Some(cache) = &self.cache else {
            return hmtx.advance(id).map_or(0, i32::from);
        };
        if let Some(advance) = cache.get(glyph) {
            return advance as i32;
        }
        let advance = hmtx
            .advance(id)
            .map_or(0, |base| i32::from(base) + self.delta(id, coords));
        cache.set(glyph, advance as u32);
        advance
    }

    pub(crate) fn fill(
        &self,
        infos: &[GlyphInfo],
        positions: &mut [GlyphPosition],
        coords: &[F2Dot14],
    ) {
        match (self.hmtx, &self.cache) {
            (Some(hmtx), None) => {
                for (info, position) in infos.iter().zip(positions) {
                    position.x_advance = hmtx
                        .advance(GlyphId(info.codepoint as u16))
                        .map_or(0, i32::from);
                }
            }
            _ => {
                for (info, position) in infos.iter().zip(positions) {
                    position.x_advance = self.advance(info.codepoint, coords);
                }
            }
        }
    }

    fn delta(&self, glyph: GlyphId, coords: &[F2Dot14]) -> i32 {
        match self
            .deltas
            .get_or_init(|| AdvanceDeltas::detect(&self.font, coords))
        {
            AdvanceDeltas::Static => 0,
            AdvanceDeltas::Metrics(variations, scalars) => {
                variations.advance_delta_fixed(scalars, glyph)
            }
            AdvanceDeltas::Phantom => self
                .font
                .phantom_advances(coords)
                .map_or(0, |phantom| phantom.advance_delta_fixed(glyph)),
        }
    }
}

impl AdvanceDeltas {
    fn detect(font: &Font<'static>, coords: &[F2Dot14]) -> Self {
        if let Some(hvar) = font.hvar() {
            let scalars = Box::new(hvar.store().fixed_scalars(coords));
            return Self::Metrics(hvar, scalars);
        }
        match font.phantom_advances(coords) {
            Some(_) => Self::Phantom,
            None => Self::Static,
        }
    }
}
