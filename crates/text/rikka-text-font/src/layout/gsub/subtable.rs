use crate::layout::chained_sequence_context::ChainedSequenceContext;
use crate::layout::coverage::Coverage;
use crate::layout::gsub::alternate::AlternateSubst;
use crate::layout::gsub::ligature::LigatureSubst;
use crate::layout::gsub::multiple::MultipleSubst;
use crate::layout::gsub::reverse_chain::ReverseChainSingleSubst;
use crate::layout::gsub::single::SingleSubst;
use crate::layout::sequence_context::SequenceContext;

#[derive(Clone, Copy, Debug)]
pub enum SubstSubtable<'a> {
    Single(SingleSubst<'a>),
    Multiple(MultipleSubst<'a>),
    Alternate(AlternateSubst<'a>),
    Ligature(LigatureSubst<'a>),
    Context(SequenceContext<'a>),
    ChainContext(ChainedSequenceContext<'a>),
    ReverseChainSingle(ReverseChainSingleSubst<'a>),
}

impl<'a> SubstSubtable<'a> {
    pub(crate) const EXTENSION: u16 = 7;

    #[inline(always)]
    pub(crate) fn new(kind: u16, data: &'a [u8]) -> Option<Self> {
        Some(match kind {
            1 => Self::Single(SingleSubst::new(data)?),
            2 => Self::Multiple(MultipleSubst::new(data)?),
            3 => Self::Alternate(AlternateSubst::new(data)?),
            4 => Self::Ligature(LigatureSubst::new(data)?),
            5 => Self::Context(SequenceContext::new(data)?),
            6 => Self::ChainContext(ChainedSequenceContext::new(data)?),
            8 => Self::ReverseChainSingle(ReverseChainSingleSubst::new(data)?),
            _ => return None,
        })
    }

    #[inline]
    pub fn coverage(&self) -> Option<Coverage<'a>> {
        match self {
            Self::Single(subtable) => Some(subtable.coverage()),
            Self::Multiple(subtable) => Some(subtable.coverage()),
            Self::Alternate(subtable) => Some(subtable.coverage()),
            Self::Ligature(subtable) => Some(subtable.coverage()),
            Self::Context(subtable) => subtable.coverage(),
            Self::ChainContext(subtable) => subtable.coverage(),
            Self::ReverseChainSingle(subtable) => Some(subtable.coverage()),
        }
    }
}
