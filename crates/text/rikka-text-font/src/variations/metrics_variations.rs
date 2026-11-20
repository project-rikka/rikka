use crate::parse::bytes::Bytes;
use crate::parse::f2dot14::F2Dot14;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::sfnt::table_id::TableId;
use crate::variations::delta_set_index_map::DeltaSetIndexMap;
use crate::variations::item_variation_store::{DeltaSetIndex, ItemVariationStore, RegionScalars};

#[derive(Clone, Copy, Debug)]
pub struct MetricsVariations<'a> {
    store: ItemVariationStore<'a>,
    advance_map: Option<DeltaSetIndexMap<'a>>,
    leading_map: Option<DeltaSetIndexMap<'a>>,
    trailing_map: Option<DeltaSetIndexMap<'a>>,
    origin_map: Option<DeltaSetIndexMap<'a>>,
}

impl<'a> MetricsVariations<'a> {
    fn new(data: &'a [u8], vertical: bool) -> Option<Self> {
        if data.u16_at(0)? != 1 {
            return None;
        }
        let map = |at: usize| data.offset32(0, at).and_then(DeltaSetIndexMap::new);
        Some(Self {
            store: ItemVariationStore::new(data.offset32(0, 4)?)?,
            advance_map: map(8),
            leading_map: map(12),
            trailing_map: map(16),
            origin_map: if vertical { map(20) } else { None },
        })
    }

    #[inline]
    pub fn store(&self) -> ItemVariationStore<'a> {
        self.store
    }

    #[inline]
    pub fn advance_delta(&self, glyph: GlyphId, coords: &[F2Dot14]) -> f32 {
        self.advance_index(glyph)
            .map_or(0.0, |index| self.store.delta(index, coords))
    }

    #[inline]
    pub fn advance_delta_fixed(&self, scalars: &RegionScalars, glyph: GlyphId) -> i32 {
        self.advance_index(glyph)
            .map_or(0, |index| self.store.delta_fixed(scalars, index))
    }

    #[inline]
    fn advance_index(&self, glyph: GlyphId) -> Option<DeltaSetIndex> {
        match self.advance_map {
            Some(map) => map.get(u32::from(glyph.0)),
            None => Some(DeltaSetIndex {
                outer: 0,
                inner: glyph.0,
            }),
        }
    }

    #[inline]
    pub fn leading_bearing_delta(&self, glyph: GlyphId, coords: &[F2Dot14]) -> Option<f32> {
        Self::mapped_delta(self.store, self.leading_map?, glyph, coords)
    }

    #[inline]
    pub fn trailing_bearing_delta(&self, glyph: GlyphId, coords: &[F2Dot14]) -> Option<f32> {
        Self::mapped_delta(self.store, self.trailing_map?, glyph, coords)
    }

    #[inline]
    pub fn vertical_origin_delta(&self, glyph: GlyphId, coords: &[F2Dot14]) -> Option<f32> {
        Self::mapped_delta(self.store, self.origin_map?, glyph, coords)
    }

    fn mapped_delta(
        store: ItemVariationStore<'a>,
        map: DeltaSetIndexMap<'a>,
        glyph: GlyphId,
        coords: &[F2Dot14],
    ) -> Option<f32> {
        Some(store.delta(map.get(u32::from(glyph.0))?, coords))
    }
}

impl<'a> Font<'a> {
    pub fn hvar(&self) -> Option<MetricsVariations<'a>> {
        MetricsVariations::new(self.table_data(TableId::Hvar)?, false)
    }

    pub fn vvar(&self) -> Option<MetricsVariations<'a>> {
        MetricsVariations::new(self.table_data(TableId::Vvar)?, true)
    }
}
