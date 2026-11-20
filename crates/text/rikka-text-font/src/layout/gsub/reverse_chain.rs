use crate::layout::coverage::Coverage;
use crate::layout::coverage_array::CoverageArray;
use crate::parse::array::Array;
use crate::parse::bytes::Bytes;
use crate::parse::cursor::Cursor;
use crate::sfnt::glyph_id::GlyphId;

#[derive(Clone, Copy, Debug)]
pub struct ReverseChainSingleSubst<'a> {
    coverage: Coverage<'a>,
    backtrack: CoverageArray<'a>,
    lookahead: CoverageArray<'a>,
    substitutes: Array<'a, GlyphId>,
}

impl<'a> ReverseChainSingleSubst<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        if data.u16_at(0)? != 1 {
            return None;
        }
        let coverage = Coverage::new(data.offset16(0, 2)?)?;
        let mut cursor = Cursor::from_position(data, 4);
        let backtrack_count = usize::from(cursor.u16()?);
        let backtrack = cursor.array(backtrack_count)?;
        let lookahead_count = usize::from(cursor.u16()?);
        let lookahead = cursor.array(lookahead_count)?;
        let substitute_count = usize::from(cursor.u16()?);
        let substitutes = cursor.array(substitute_count)?;
        Some(Self {
            coverage,
            backtrack: CoverageArray::new(data, backtrack),
            lookahead: CoverageArray::new(data, lookahead),
            substitutes,
        })
    }

    #[inline]
    pub fn coverage(&self) -> Coverage<'a> {
        self.coverage
    }

    #[inline]
    pub fn backtrack(&self) -> CoverageArray<'a> {
        self.backtrack
    }

    #[inline]
    pub fn lookahead(&self) -> CoverageArray<'a> {
        self.lookahead
    }

    #[inline]
    pub fn substitute_at(&self, coverage_index: u16) -> Option<GlyphId> {
        self.substitutes.get(usize::from(coverage_index))
    }
}
