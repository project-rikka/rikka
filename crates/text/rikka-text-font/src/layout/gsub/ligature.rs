use crate::layout::coverage::Coverage;
use crate::layout::offset_list::OffsetList;
use crate::parse::array::Array;
use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;

#[derive(Clone, Copy, Debug)]
pub struct LigatureSubst<'a> {
    data: &'a [u8],
    sets: OffsetList<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct LigatureSet<'a> {
    ligatures: OffsetList<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct Ligature<'a> {
    glyph: GlyphId,
    components: Array<'a, GlyphId>,
}

impl<'a> LigatureSubst<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        if data.u16_at(0)? != 1 {
            return None;
        }
        Some(Self {
            data,
            sets: OffsetList::new(data, 4)?,
        })
    }

    #[inline]
    pub fn coverage(&self) -> Coverage<'a> {
        Coverage::from_offset(self.data, 2)
    }

    #[inline]
    pub fn ligature_set_at(&self, coverage_index: u16) -> Option<LigatureSet<'a>> {
        Some(LigatureSet {
            ligatures: OffsetList::new(self.sets.get(usize::from(coverage_index))?, 0)?,
        })
    }

    #[inline]
    pub fn ligature_set(&self, first: GlyphId) -> Option<LigatureSet<'a>> {
        self.ligature_set_at(self.coverage().get(first)?)
    }
}

impl<'a> LigatureSet<'a> {
    #[inline]
    pub fn len(&self) -> usize {
        self.ligatures.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.ligatures.len() == 0
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<Ligature<'a>> {
        let data = self.ligatures.get(index)?;
        let glyph = GlyphId(data.u16_at(0)?);
        let count = usize::from(data.u16_at(2)?);
        Some(Ligature {
            glyph,
            components: data.array(4, count.checked_sub(1)?)?,
        })
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = Ligature<'a>> + use<'a> {
        let set = *self;
        (0..self.len()).filter_map(move |index| set.get(index))
    }
}

impl<'a> Ligature<'a> {
    #[inline]
    pub fn glyph(&self) -> GlyphId {
        self.glyph
    }

    #[inline]
    pub fn components(&self) -> Array<'a, GlyphId> {
        self.components
    }

    #[inline]
    pub fn component_count(&self) -> usize {
        self.components.len() + 1
    }
}
