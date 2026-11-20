use crate::layout::class_def::ClassDef;
use crate::layout::coverage::Coverage;
use crate::layout::coverage_array::CoverageArray;
use crate::layout::offset_list::OffsetList;
use crate::parse::array::{Array, Scalar};
use crate::parse::bytes::Bytes;
use crate::parse::cursor::Cursor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SequenceLookupRecord {
    pub sequence_index: u16,
    pub lookup_index: u16,
}

impl Scalar for SequenceLookupRecord {
    const SIZE: usize = 4;

    #[inline]
    fn parse(bytes: &[u8]) -> Option<Self> {
        Some(Self {
            sequence_index: bytes.u16_at(0)?,
            lookup_index: bytes.u16_at(2)?,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SequenceContext<'a> {
    Glyphs(GlyphSequenceContext<'a>),
    Classes(ClassSequenceContext<'a>),
    Coverages(CoverageSequenceContext<'a>),
}

#[derive(Clone, Copy, Debug)]
pub struct GlyphSequenceContext<'a> {
    data: &'a [u8],
    rule_sets: OffsetList<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct ClassSequenceContext<'a> {
    data: &'a [u8],
    rule_sets: OffsetList<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct CoverageSequenceContext<'a> {
    input: CoverageArray<'a>,
    lookups: Array<'a, SequenceLookupRecord>,
}

#[derive(Clone, Copy, Debug)]
pub struct SequenceRuleSet<'a> {
    rules: OffsetList<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct SequenceRule<'a> {
    input: Array<'a, u16>,
    lookups: Array<'a, SequenceLookupRecord>,
}

impl<'a> SequenceContext<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        match data.u16_at(0)? {
            1 => Some(Self::Glyphs(GlyphSequenceContext {
                data,
                rule_sets: OffsetList::new(data, 4)?,
            })),
            2 => Some(Self::Classes(ClassSequenceContext {
                data,
                rule_sets: OffsetList::new(data, 6)?,
            })),
            3 => {
                let mut cursor = Cursor::from_position(data, 2);
                let glyph_count = usize::from(cursor.u16()?);
                let lookup_count = usize::from(cursor.u16()?);
                let coverages = cursor.array(glyph_count)?;
                let lookups = cursor.array(lookup_count)?;
                Some(Self::Coverages(CoverageSequenceContext {
                    input: CoverageArray::new(data, coverages),
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

impl<'a> GlyphSequenceContext<'a> {
    #[inline]
    pub fn coverage(&self) -> Coverage<'a> {
        Coverage::from_offset(self.data, 2)
    }

    #[inline]
    pub fn rule_set(&self, coverage_index: u16) -> Option<SequenceRuleSet<'a>> {
        SequenceRuleSet::new(self.rule_sets.get(usize::from(coverage_index))?)
    }
}

impl<'a> ClassSequenceContext<'a> {
    #[inline]
    pub fn coverage(&self) -> Coverage<'a> {
        Coverage::from_offset(self.data, 2)
    }

    #[inline]
    pub fn class_def(&self) -> ClassDef<'a> {
        ClassDef::from_offset(self.data, 4)
    }

    #[inline]
    pub fn rule_set(&self, class: u16) -> Option<SequenceRuleSet<'a>> {
        SequenceRuleSet::new(self.rule_sets.get(usize::from(class))?)
    }
}

impl<'a> CoverageSequenceContext<'a> {
    #[inline]
    pub fn input(&self) -> CoverageArray<'a> {
        self.input
    }

    #[inline]
    pub fn lookups(&self) -> Array<'a, SequenceLookupRecord> {
        self.lookups
    }
}

impl<'a> SequenceRuleSet<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
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
    pub fn get(&self, index: usize) -> Option<SequenceRule<'a>> {
        SequenceRule::new(self.rules.get(index)?)
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = SequenceRule<'a>> + use<'a> {
        let set = *self;
        (0..self.len()).filter_map(move |index| set.get(index))
    }
}

impl<'a> SequenceRule<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        let mut cursor = Cursor::new(data);
        let glyph_count = usize::from(cursor.u16()?);
        let lookup_count = usize::from(cursor.u16()?);
        let input = cursor.array(glyph_count.checked_sub(1)?)?;
        let lookups = cursor.array(lookup_count)?;
        Some(Self { input, lookups })
    }

    #[inline]
    pub fn input(&self) -> Array<'a, u16> {
        self.input
    }

    #[inline]
    pub fn lookups(&self) -> Array<'a, SequenceLookupRecord> {
        self.lookups
    }
}
