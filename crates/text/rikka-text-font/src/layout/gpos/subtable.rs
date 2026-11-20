use crate::layout::chained_sequence_context::ChainedSequenceContext;
use crate::layout::coverage::Coverage;
use crate::layout::gpos::cursive::CursivePos;
use crate::layout::gpos::mark_base::MarkBasePos;
use crate::layout::gpos::mark_ligature::MarkLigPos;
use crate::layout::gpos::mark_mark::MarkMarkPos;
use crate::layout::gpos::pair::PairPos;
use crate::layout::gpos::single::SinglePos;
use crate::layout::sequence_context::SequenceContext;

#[derive(Clone, Copy, Debug)]
pub enum PosSubtable<'a> {
    Single(SinglePos<'a>),
    Pair(PairPos<'a>),
    Cursive(CursivePos<'a>),
    MarkBase(MarkBasePos<'a>),
    MarkLigature(MarkLigPos<'a>),
    MarkMark(MarkMarkPos<'a>),
    Context(SequenceContext<'a>),
    ChainContext(ChainedSequenceContext<'a>),
}

impl<'a> PosSubtable<'a> {
    pub(crate) const EXTENSION: u16 = 9;

    #[inline(always)]
    pub(crate) fn new(kind: u16, data: &'a [u8]) -> Option<Self> {
        Some(match kind {
            1 => Self::Single(SinglePos::new(data)?),
            2 => Self::Pair(PairPos::new(data)?),
            3 => Self::Cursive(CursivePos::new(data)?),
            4 => Self::MarkBase(MarkBasePos::new(data)?),
            5 => Self::MarkLigature(MarkLigPos::new(data)?),
            6 => Self::MarkMark(MarkMarkPos::new(data)?),
            7 => Self::Context(SequenceContext::new(data)?),
            8 => Self::ChainContext(ChainedSequenceContext::new(data)?),
            _ => return None,
        })
    }

    #[inline]
    pub fn coverage(&self) -> Option<Coverage<'a>> {
        match self {
            Self::Single(subtable) => Some(subtable.coverage()),
            Self::Pair(subtable) => Some(subtable.coverage()),
            Self::Cursive(subtable) => Some(subtable.coverage()),
            Self::MarkBase(subtable) => Some(subtable.mark_coverage()),
            Self::MarkLigature(subtable) => Some(subtable.mark_coverage()),
            Self::MarkMark(subtable) => Some(subtable.mark_coverage()),
            Self::Context(subtable) => subtable.coverage(),
            Self::ChainContext(subtable) => subtable.coverage(),
        }
    }
}
