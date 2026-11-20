use crate::layout::class_def::ClassDef;
use crate::layout::coverage::Coverage;
use crate::layout::coverage_array::CoverageArray;
use crate::layout::offset_list::OffsetList;
use crate::layout::sequence_context::SequenceLookupRecord;
use crate::parse::array::Array;
use crate::parse::bytes::Bytes;
use crate::parse::cursor::Cursor;

#[derive(Clone, Copy, Debug)]
pub enum ChainedSequenceContext<'a> {
    Glyphs(ChainedGlyphSequenceContext<'a>),
    Classes(ChainedClassSequenceContext<'a>),
    Coverages(ChainedCoverageSequenceContext<'a>),
}

#[derive(Clone, Copy, Debug)]
pub struct ChainedGlyphSequenceContext<'a> {
    data: &'a [u8],
    rule_sets: OffsetList<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct ChainedClassSequenceContext<'a> {
    data: &'a [u8],
    rule_sets: OffsetList<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct ChainedCoverageSequenceContext<'a> {
    backtrack: CoverageArray<'a>,
    input: CoverageArray<'a>,
    lookahead: CoverageArray<'a>,
    lookups: Array<'a, SequenceLookupRecord>,
}

#[derive(Clone, Copy, Debug)]
pub struct ChainedSequenceRuleSet<'a> {
    rules: OffsetList<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct ChainedSequenceRule<'a> {
    backtrack: Array<'a, u16>,
    input: Array<'a, u16>,
    lookahead: Array<'a, u16>,
    lookups: Array<'a, SequenceLookupRecord>,
}

impl<'a> ChainedSequenceContext<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        match data.u16_at(0)? {
            1 => Some(Self::Glyphs(ChainedGlyphSequenceContext {
                data,
                rule_sets: OffsetList::new(data, 4)?,
            })),
            2 => Some(Self::Classes(ChainedClassSequenceContext {
                data,
                rule_sets: OffsetList::new(data, 10)?,
            })),
            3 => {
                let mut cursor = Cursor::from_position(data, 2);
                let backtrack_count = usize::from(cursor.u16()?);
                let backtrack = cursor.array(backtrack_count)?;
                let input_count = usize::from(cursor.u16()?);
                let input = cursor.array(input_count)?;
                let lookahead_count = usize::from(cursor.u16()?);
                let lookahead = cursor.array(lookahead_count)?;
                let lookup_count = usize::from(cursor.u16()?);
                let lookups = cursor.array(lookup_count)?;
                Some(Self::Coverages(ChainedCoverageSequenceContext {
                    backtrack: CoverageArray::new(data, backtrack),
                    input: CoverageArray::new(data, input),
                    lookahead: CoverageArray::new(data, lookahead),
                    lookups,
                }))
            }
            _ => None,
        }
    }

    #[inline]
    pub fn coverage(&self) -> Option<Coverage<'a>> {
        match self {
            Self::Glyphs(context) => Some(context.coverage()),
            Self::Classes(context) => Some(context.coverage()),
            Self::Coverages(context) => context.input.get(0),
        }
    }
}

impl<'a> ChainedGlyphSequenceContext<'a> {
    #[inline]
    pub fn coverage(&self) -> Coverage<'a> {
        Coverage::from_offset(self.data, 2)
    }

    #[inline]
    pub fn rule_set(&self, coverage_index: u16) -> Option<ChainedSequenceRuleSet<'a>> {
        ChainedSequenceRuleSet::new(self.rule_sets.get(usize::from(coverage_index))?)
    }
}

impl<'a> ChainedClassSequenceContext<'a> {
    #[inline]
    pub fn coverage(&self) -> Coverage<'a> {
        Coverage::from_offset(self.data, 2)
    }

    #[inline]
    pub fn backtrack_class_def(&self) -> ClassDef<'a> {
        ClassDef::from_offset(self.data, 4)
    }

    #[inline]
    pub fn input_class_def(&self) -> ClassDef<'a> {
        ClassDef::from_offset(self.data, 6)
    }

    #[inline]
    pub fn lookahead_class_def(&self) -> ClassDef<'a> {
        ClassDef::from_offset(self.data, 8)
    }

    #[inline]
    pub fn rule_set(&self, class: u16) -> Option<ChainedSequenceRuleSet<'a>> {
        ChainedSequenceRuleSet::new(self.rule_sets.get(usize::from(class))?)
    }
}

impl<'a> ChainedCoverageSequenceContext<'a> {
    #[inline]
    pub fn backtrack(&self) -> CoverageArray<'a> {
        self.backtrack
    }

    #[inline]
    pub fn input(&self) -> CoverageArray<'a> {
        self.input
    }

    #[inline]
    pub fn lookahead(&self) -> CoverageArray<'a> {
        self.lookahead
    }

    #[inline]
    pub fn lookups(&self) -> Array<'a, SequenceLookupRecord> {
        self.lookups
    }
}

impl<'a> ChainedSequenceRuleSet<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        Some(Self {
            rules: OffsetList::new(data, 0)?,
        })
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rules.len() == 0
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<ChainedSequenceRule<'a>> {
        ChainedSequenceRule::new(self.rules.get(index)?)
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = ChainedSequenceRule<'a>> + use<'a> {
        let set = *self;
        (0..self.len()).filter_map(move |index| set.get(index))
    }
}

impl<'a> ChainedSequenceRule<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        let mut cursor = Cursor::new(data);
        let backtrack_count = usize::from(cursor.u16()?);
        let backtrack = cursor.array(backtrack_count)?;
        let input_count = usize::from(cursor.u16()?);
        let input = cursor.array(input_count.checked_sub(1)?)?;
        let lookahead_count = usize::from(cursor.u16()?);
        let lookahead = cursor.array(lookahead_count)?;
        let lookup_count = usize::from(cursor.u16()?);
        let lookups = cursor.array(lookup_count)?;
        Some(Self {
            backtrack,
            input,
            lookahead,
            lookups,
        })
    }

    #[inline]
    pub fn backtrack(&self) -> Array<'a, u16> {
        self.backtrack
    }

    #[inline]
    pub fn input(&self) -> Array<'a, u16> {
        self.input
    }

    #[inline]
    pub fn lookahead(&self) -> Array<'a, u16> {
        self.lookahead
    }

    #[inline]
    pub fn lookups(&self) -> Array<'a, SequenceLookupRecord> {
        self.lookups
    }
}
