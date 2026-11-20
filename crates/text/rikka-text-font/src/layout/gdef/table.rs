use crate::layout::class_def::ClassDef;
use crate::layout::coverage::Coverage;
use crate::layout::gdef::caret::LigatureCarets;
use crate::layout::gdef::glyph_class::GlyphClass;
use crate::layout::offset_list::OffsetList;
use crate::parse::array::Array;
use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::sfnt::table_id::TableId;
use crate::variations::item_variation_store::ItemVariationStore;

#[derive(Clone, Copy, Debug)]
pub struct Gdef<'a> {
    data: &'a [u8],
    glyph_classes: Option<ClassDef<'a>>,
    mark_attach_classes: ClassDef<'a>,
    mark_glyph_sets: Option<MarkGlyphSets<'a>>,
}

#[derive(Clone, Copy, Debug)]
pub struct MarkGlyphSets<'a> {
    data: &'a [u8],
    count: u16,
}

impl<'a> Gdef<'a> {
    #[inline]
    pub fn new(data: &'a [u8]) -> Option<Self> {
        if data.u16_at(0)? != 1 {
            return None;
        }
        let minor = data.u16_at(2)?;
        let mark_glyph_sets = if minor >= 2 {
            data.offset16(0, 12).and_then(MarkGlyphSets::new)
        } else {
            None
        };
        Some(Self {
            data,
            glyph_classes: data.offset16(0, 4).and_then(ClassDef::new),
            mark_attach_classes: ClassDef::or_empty(data.offset16(0, 10)),
            mark_glyph_sets,
        })
    }

    #[inline]
    pub fn has_glyph_classes(&self) -> bool {
        self.glyph_classes.is_some()
    }

    #[inline]
    pub fn glyph_class_def(&self) -> ClassDef<'a> {
        self.glyph_classes.unwrap_or_default()
    }

    #[inline]
    pub fn glyph_class(&self, glyph: GlyphId) -> GlyphClass {
        self.glyph_classes
            .map_or(GlyphClass::Unclassified, |classes| {
                GlyphClass::from_raw(classes.get(glyph))
            })
    }

    #[inline]
    pub fn mark_attach_class_def(&self) -> ClassDef<'a> {
        self.mark_attach_classes
    }

    #[inline]
    pub fn mark_attach_class(&self, glyph: GlyphId) -> u16 {
        self.mark_attach_classes.get(glyph)
    }

    #[inline]
    pub fn mark_glyph_sets(&self) -> Option<MarkGlyphSets<'a>> {
        self.mark_glyph_sets
    }

    #[inline]
    pub fn mark_set_covers(&self, set: u16, glyph: GlyphId) -> bool {
        self.mark_glyph_sets
            .is_some_and(|sets| sets.covers(set, glyph))
    }

    #[inline]
    pub fn attach_points(&self, glyph: GlyphId) -> Option<Array<'a, u16>> {
        let list = self.data.offset16(0, 6)?;
        let index = Coverage::new(list.offset16(0, 0)?)?.get(glyph)?;
        let points = OffsetList::new(list, 2)?.get(usize::from(index))?;
        let count = usize::from(points.u16_at(0)?);
        points.array(2, count)
    }

    #[inline]
    pub fn ligature_carets(&self, glyph: GlyphId) -> Option<LigatureCarets<'a>> {
        let list = self.data.offset16(0, 8)?;
        let index = Coverage::new(list.offset16(0, 0)?)?.get(glyph)?;
        LigatureCarets::new(OffsetList::new(list, 2)?.get(usize::from(index))?)
    }

    #[inline]
    pub fn var_store(&self) -> Option<ItemVariationStore<'a>> {
        if self.data.u16_at(2)? < 3 {
            return None;
        }
        ItemVariationStore::new(self.data.offset32(0, 14)?)
    }
}

impl<'a> MarkGlyphSets<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        if data.u16_at(0)? != 1 {
            return None;
        }
        let count = data.u16_at(2)?;
        data.range(4, usize::from(count) * 4)?;
        Some(Self { data, count })
    }

    #[inline]
    pub fn len(&self) -> usize {
        usize::from(self.count)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    #[inline]
    pub fn get(&self, set: u16) -> Option<Coverage<'a>> {
        if set >= self.count {
            return None;
        }
        Coverage::new(self.data.offset32(0, 4 + usize::from(set) * 4)?)
    }

    #[inline]
    pub fn covers(&self, set: u16, glyph: GlyphId) -> bool {
        self.get(set)
            .is_some_and(|coverage| coverage.contains(glyph))
    }
}

impl<'a> Font<'a> {
    #[inline]
    pub fn gdef(&self) -> Option<Gdef<'a>> {
        Gdef::new(self.table_data(TableId::Gdef)?)
    }
}
