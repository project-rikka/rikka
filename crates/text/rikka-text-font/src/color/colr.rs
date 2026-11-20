use std::cmp::Ordering;

use crate::color::clip_list::ClipList;
use crate::color::color_value::ColorIndex;
use crate::color::colr_variations::ColrVariations;
use crate::color::paint::{Paint, PaintId, parse_paint};
use crate::outline::bounds::Bounds;
use crate::parse::bytes::Bytes;
use crate::parse::f2dot14::F2Dot14;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::sfnt::table_id::TableId;
use crate::variations::delta_set_index_map::DeltaSetIndexMap;
use crate::variations::item_variation_store::ItemVariationStore;

const BASE_GLYPH_RECORD_SIZE: usize = 6;
const LAYER_RECORD_SIZE: usize = 4;
const BASE_GLYPH_PAINT_RECORD_SIZE: usize = 6;

#[derive(Clone, Copy, Debug)]
pub struct Colr<'a> {
    data: &'a [u8],
    version: u16,
    base_glyphs: &'a [u8],
    layers: &'a [u8],
    base_glyph_list: Option<u32>,
    base_glyph_paint_count: usize,
    layer_list: Option<u32>,
    layer_paint_count: usize,
    clip_list: Option<ClipList<'a>>,
    var_index_map: Option<DeltaSetIndexMap<'a>>,
    var_store: Option<ItemVariationStore<'a>>,
}

#[derive(Clone, Copy, Debug)]
pub enum ColorGlyphPaint {
    Layers { first: u16, count: u16 },
    Graph(PaintId),
}

#[derive(Clone, Copy, Debug)]
pub struct ColorGlyph<'a> {
    pub(crate) colr: Colr<'a>,
    pub(crate) glyph: GlyphId,
    pub(crate) paint: ColorGlyphPaint,
}

impl<'a> Colr<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        let version = data.u16_at(0)?;
        if version > 1 {
            return None;
        }
        let base_glyph_count = usize::from(data.u16_at(2)?);
        let layer_count = usize::from(data.u16_at(12)?);
        let base_glyphs = data
            .range(
                data.u32_at(4)? as usize,
                base_glyph_count * BASE_GLYPH_RECORD_SIZE,
            )
            .unwrap_or_default();
        let layers = data
            .range(data.u32_at(8)? as usize, layer_count * LAYER_RECORD_SIZE)
            .unwrap_or_default();
        let mut colr = Self {
            data,
            version,
            base_glyphs,
            layers,
            base_glyph_list: None,
            base_glyph_paint_count: 0,
            layer_list: None,
            layer_paint_count: 0,
            clip_list: None,
            var_index_map: None,
            var_store: None,
        };
        if version == 1 {
            colr.read_v1_header();
        }
        Some(colr)
    }

    fn read_v1_header(&mut self) {
        let data = self.data;
        let counted = |at: usize, record_size: usize| -> Option<(u32, usize)> {
            let offset = data.u32_at(at).filter(|&offset| offset != 0)?;
            let count = data.u32_at(offset as usize)? as usize;
            data.range(offset as usize + 4, count.checked_mul(record_size)?)?;
            Some((offset, count))
        };
        if let Some((offset, count)) = counted(14, BASE_GLYPH_PAINT_RECORD_SIZE) {
            self.base_glyph_list = Some(offset);
            self.base_glyph_paint_count = count;
        }
        if let Some((offset, count)) = counted(18, 4) {
            self.layer_list = Some(offset);
            self.layer_paint_count = count;
        }
        self.clip_list = data.offset32(0, 22).and_then(ClipList::new);
        self.var_index_map = data.offset32(0, 26).and_then(DeltaSetIndexMap::new);
        self.var_store = data.offset32(0, 30).and_then(ItemVariationStore::new);
    }

    #[inline]
    pub(crate) fn data(&self) -> &'a [u8] {
        self.data
    }

    #[inline]
    pub fn version(&self) -> u16 {
        self.version
    }

    pub fn color_glyph(&self, glyph: GlyphId) -> Option<ColorGlyph<'a>> {
        let paint = match self.base_glyph_paint(glyph) {
            Some(id) => ColorGlyphPaint::Graph(id),
            None => {
                let (first, count) = self.base_glyph_layers(glyph)?;
                ColorGlyphPaint::Layers { first, count }
            }
        };
        Some(ColorGlyph {
            colr: *self,
            glyph,
            paint,
        })
    }

    pub fn has_color_glyph(&self, glyph: GlyphId) -> bool {
        self.base_glyph_paint(glyph).is_some() || self.base_glyph_layers(glyph).is_some()
    }

    fn base_glyph_layers(&self, glyph: GlyphId) -> Option<(u16, u16)> {
        let count = self.base_glyphs.len() / BASE_GLYPH_RECORD_SIZE;
        let index = binary_search(count, |index| {
            Some(
                self.base_glyphs
                    .u16_at(index * BASE_GLYPH_RECORD_SIZE)?
                    .cmp(&glyph.0),
            )
        })?;
        let record = index * BASE_GLYPH_RECORD_SIZE;
        let first = self.base_glyphs.u16_at(record + 2)?;
        let count = self.base_glyphs.u16_at(record + 4)?;
        Some((first, count))
    }

    pub fn layer(&self, index: u16) -> Option<(GlyphId, ColorIndex)> {
        let record = usize::from(index) * LAYER_RECORD_SIZE;
        Some((
            GlyphId(self.layers.u16_at(record)?),
            ColorIndex::from_raw(self.layers.u16_at(record + 2)?),
        ))
    }

    pub fn base_glyph_paint(&self, glyph: GlyphId) -> Option<PaintId> {
        let list = self.base_glyph_list? as usize;
        let records = list + 4;
        let index = binary_search(self.base_glyph_paint_count, |index| {
            Some(
                self.data
                    .u16_at(records + index * BASE_GLYPH_PAINT_RECORD_SIZE)?
                    .cmp(&glyph.0),
            )
        })?;
        let offset = self
            .data
            .u32_at(records + index * BASE_GLYPH_PAINT_RECORD_SIZE + 2)?;
        self.paint_id(list, offset)
    }

    #[inline]
    pub fn layer_paint_count(&self) -> u32 {
        self.layer_paint_count as u32
    }

    pub fn layer_paint(&self, index: u32) -> Option<PaintId> {
        let list = self.layer_list? as usize;
        if index as usize >= self.layer_paint_count {
            return None;
        }
        let offset = self.data.u32_at(list + 4 + index as usize * 4)?;
        self.paint_id(list, offset)
    }

    fn paint_id(&self, base: usize, offset: u32) -> Option<PaintId> {
        if offset == 0 {
            return None;
        }
        let at = u32::try_from(base).ok()?.checked_add(offset)?;
        ((at as usize) < self.data.len()).then_some(PaintId(at))
    }

    pub fn paint<'c>(&self, id: PaintId, coords: &'c [F2Dot14]) -> Option<Paint<'c>>
    where
        'a: 'c,
    {
        parse_paint(self.data, id, self.variations(coords))
    }

    pub fn clip_box(&self, glyph: GlyphId, coords: &[F2Dot14]) -> Option<Bounds> {
        self.clip_list?.clip_box(glyph, &self.variations(coords))
    }

    pub(crate) fn variations<'c>(&self, coords: &'c [F2Dot14]) -> ColrVariations<'c>
    where
        'a: 'c,
    {
        ColrVariations::new(self.var_store, self.var_index_map, coords)
    }
}

fn binary_search(
    count: usize,
    mut compare: impl FnMut(usize) -> Option<Ordering>,
) -> Option<usize> {
    let (mut low, mut high) = (0, count);
    while low < high {
        let middle = low + (high - low) / 2;
        match compare(middle)? {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => high = middle,
            Ordering::Equal => return Some(middle),
        }
    }
    None
}

impl<'a> ColorGlyph<'a> {
    #[inline]
    pub fn glyph(&self) -> GlyphId {
        self.glyph
    }

    #[inline]
    pub fn paint_root(&self) -> ColorGlyphPaint {
        self.paint
    }

    pub fn clip_box(&self, coords: &[F2Dot14]) -> Option<Bounds> {
        match self.paint {
            ColorGlyphPaint::Graph(_) => self.colr.clip_box(self.glyph, coords),
            ColorGlyphPaint::Layers { .. } => None,
        }
    }
}

impl<'a> Font<'a> {
    pub fn colr(&self) -> Option<Colr<'a>> {
        Colr::new(self.table_data(TableId::Colr)?)
    }

    pub fn color_glyph(&self, glyph: GlyphId) -> Option<ColorGlyph<'a>> {
        self.colr()?.color_glyph(glyph)
    }
}
