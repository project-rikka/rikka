use crate::layout::coverage::Coverage;
use crate::layout::gpos::anchor::Anchor;
use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;

#[derive(Clone, Copy, Debug)]
pub struct CursivePos<'a> {
    data: &'a [u8],
    records: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct EntryExit<'a> {
    pub entry: Option<Anchor<'a>>,
    pub exit: Option<Anchor<'a>>,
}

impl<'a> CursivePos<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        if data.u16_at(0)? != 1 {
            return None;
        }
        let count = usize::from(data.u16_at(4)?);
        Some(Self {
            data,
            records: data.range(6, count * 4)?,
        })
    }

    #[inline]
    pub fn coverage(&self) -> Coverage<'a> {
        Coverage::from_offset(self.data, 2)
    }

    #[inline]
    pub fn entry_exit_at(&self, coverage_index: u16) -> Option<EntryExit<'a>> {
        let record = usize::from(coverage_index) * 4;
        if record + 4 > self.records.len() {
            return None;
        }
        let anchor = |at: usize| {
            let offset = self.records.u16_at(at)?;
            if offset == 0 {
                return None;
            }
            Anchor::new(self.data.tail(usize::from(offset))?)
        };
        Some(EntryExit {
            entry: anchor(record),
            exit: anchor(record + 2),
        })
    }

    #[inline]
    pub fn entry_exit(&self, glyph: GlyphId) -> Option<EntryExit<'a>> {
        self.entry_exit_at(self.coverage().get(glyph)?)
    }
}
