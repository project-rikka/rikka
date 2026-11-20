use crate::layout::coverage::Coverage;
use crate::layout::gsub::glyph_sequences::GlyphSequences;
use crate::parse::array::Array;
use crate::sfnt::glyph_id::GlyphId;

#[derive(Clone, Copy, Debug)]
pub struct MultipleSubst<'a> {
    sequences: GlyphSequences<'a>,
}

impl<'a> MultipleSubst<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        Some(Self {
            sequences: GlyphSequences::new(data)?,
        })
    }

    #[inline]
    pub fn coverage(&self) -> Coverage<'a> {
        self.sequences.coverage()
    }

    #[inline]
    pub fn sequence(&self, coverage_index: u16) -> Option<Array<'a, GlyphId>> {
        self.sequences.sequence(coverage_index)
    }

    #[inline]
    pub fn substitute(&self, glyph: GlyphId) -> Option<Array<'a, GlyphId>> {
        self.sequences.lookup(glyph)
    }
}
