use crate::layout::coverage::Coverage;
use crate::layout::offset_list::OffsetList;
use crate::parse::array::Array;
use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;

#[derive(Clone, Copy, Debug)]
pub(crate) struct GlyphSequences<'a> {
    data: &'a [u8],
    sequences: OffsetList<'a>,
}

impl<'a> GlyphSequences<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        if data.u16_at(0)? != 1 {
            return None;
        }
        Some(Self {
            data,
            sequences: OffsetList::new(data, 4)?,
        })
    }

    #[inline]
    pub(crate) fn coverage(&self) -> Coverage<'a> {
        Coverage::from_offset(self.data, 2)
    }

    #[inline]
    pub(crate) fn sequence(&self, index: u16) -> Option<Array<'a, GlyphId>> {
        let data = self.sequences.get(usize::from(index))?;
        let count = usize::from(data.u16_at(0)?);
        data.array(2, count)
    }

    #[inline]
    pub(crate) fn lookup(&self, glyph: GlyphId) -> Option<Array<'a, GlyphId>> {
        self.sequence(self.coverage().get(glyph)?)
    }
}
