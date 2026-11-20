use crate::layout::coverage::Coverage;
use crate::layout::gsub::glyph_sequences::GlyphSequences;
use crate::parse::array::Array;
use crate::sfnt::glyph_id::GlyphId;

#[derive(Clone, Copy, Debug)]
pub struct AlternateSubst<'a> {
    sets: GlyphSequences<'a>,
}

impl<'a> AlternateSubst<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        Some(Self {
            sets: GlyphSequences::new(data)?,
        })
    }

    #[inline]
    pub fn coverage(&self) -> Coverage<'a> {
        self.sets.coverage()
    }

    #[inline]
    pub fn alternate_set(&self, coverage_index: u16) -> Option<Array<'a, GlyphId>> {
        self.sets.sequence(coverage_index)
    }

    #[inline]
    pub fn alternates(&self, glyph: GlyphId) -> Option<Array<'a, GlyphId>> {
        self.sets.lookup(glyph)
    }
}
