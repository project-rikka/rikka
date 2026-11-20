use crate::layout::class_def::ClassDef;
use crate::layout::coverage::Coverage;
use crate::layout::gpos::value_record::{ValueFormat, ValueRecord};
use crate::layout::offset_list::OffsetList;
use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;

#[derive(Clone, Copy, Debug)]
pub enum PairPos<'a> {
    Glyphs(GlyphPairPos<'a>),
    Classes(ClassPairPos<'a>),
}

#[derive(Clone, Copy, Debug)]
pub struct GlyphPairPos<'a> {
    data: &'a [u8],
    formats: (ValueFormat, ValueFormat),
    sets: OffsetList<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct PairSet<'a> {
    data: &'a [u8],
    formats: (ValueFormat, ValueFormat),
    count: usize,
    record_size: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct ClassPairPos<'a> {
    data: &'a [u8],
    formats: (ValueFormat, ValueFormat),
    first_class_count: u16,
    second_class_count: u16,
}

impl<'a> PairPos<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        let formats = (ValueFormat(data.u16_at(4)?), ValueFormat(data.u16_at(6)?));
        match data.u16_at(0)? {
            1 => Some(Self::Glyphs(GlyphPairPos {
                data,
                formats,
                sets: OffsetList::new(data, 8)?,
            })),
            2 => {
                let first_class_count = data.u16_at(12)?;
                let second_class_count = data.u16_at(14)?;
                let record_size = formats.0.size() + formats.1.size();
                let len = usize::from(first_class_count)
                    .checked_mul(usize::from(second_class_count))?
                    .checked_mul(record_size)?;
                data.range(16, len)?;
                Some(Self::Classes(ClassPairPos {
                    data,
                    formats,
                    first_class_count,
                    second_class_count,
                }))
            }
            _ => None,
        }
    }

    #[inline]
    pub fn coverage(&self) -> Coverage<'a> {
        match self {
            Self::Glyphs(pairs) => pairs.coverage(),
            Self::Classes(pairs) => pairs.coverage(),
        }
    }

    #[inline]
    pub fn value_formats(&self) -> (ValueFormat, ValueFormat) {
        match self {
            Self::Glyphs(pairs) => pairs.formats,
            Self::Classes(pairs) => pairs.formats,
        }
    }

    #[inline]
    pub fn values(
        &self,
        first: GlyphId,
        second: GlyphId,
    ) -> Option<(ValueRecord<'a>, ValueRecord<'a>)> {
        let index = self.coverage().get(first)?;
        match self {
            Self::Glyphs(pairs) => pairs.pair_set(index)?.find(second),
            Self::Classes(pairs) => pairs.class_values(
                pairs.first_class_def().get(first),
                pairs.second_class_def().get(second),
            ),
        }
    }
}

impl<'a> GlyphPairPos<'a> {
    #[inline]
    pub fn coverage(&self) -> Coverage<'a> {
        Coverage::from_offset(self.data, 2)
    }

    #[inline]
    pub fn pair_set_count(&self) -> usize {
        self.sets.len()
    }

    #[inline]
    pub fn pair_set(&self, coverage_index: u16) -> Option<PairSet<'a>> {
        let data = self.sets.get(usize::from(coverage_index))?;
        let count = usize::from(data.u16_at(0)?);
        let record_size = 2 + self.formats.0.size() + self.formats.1.size();
        data.range(2, count.checked_mul(record_size)?)?;
        Some(PairSet {
            data,
            formats: self.formats,
            count,
            record_size,
        })
    }
}

impl<'a> PairSet<'a> {
    #[inline]
    pub fn len(&self) -> usize {
        self.count
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<(GlyphId, ValueRecord<'a>, ValueRecord<'a>)> {
        if index >= self.count {
            return None;
        }
        let record = 2 + index * self.record_size;
        let second = GlyphId(self.data.u16_at(record)?);
        let first_value = ValueRecord::new(self.formats.0, self.data, record + 2)?;
        let second_value = ValueRecord::new(
            self.formats.1,
            self.data,
            record + 2 + self.formats.0.size(),
        )?;
        Some((second, first_value, second_value))
    }

    #[inline]
    pub fn find(&self, second: GlyphId) -> Option<(ValueRecord<'a>, ValueRecord<'a>)> {
        let mut low = 0;
        let mut high = self.count;
        while low < high {
            let middle = low + (high - low) / 2;
            let glyph = self.data.u16_at(2 + middle * self.record_size)?;
            match glyph.cmp(&second.0) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => {
                    let (_, first_value, second_value) = self.get(middle)?;
                    return Some((first_value, second_value));
                }
            }
        }
        None
    }

    pub fn is_ordered(&self) -> bool {
        (1..self.count).all(|index| self.second_at(index - 1) < self.second_at(index))
    }

    #[inline]
    pub fn find_ordered(&self, second: GlyphId) -> Option<(ValueRecord<'a>, ValueRecord<'a>)> {
        if self.count == 0 {
            return None;
        }
        let mut base = 0;
        let mut size = self.count;
        while size > 1 {
            let half = size / 2;
            let middle = base + half;
            base =
                std::hint::select_unpredictable(self.second_at(middle) <= second.0, middle, base);
            size -= half;
        }
        if self.second_at(base) != second.0 {
            return None;
        }
        let (_, first_value, second_value) = self.get(base)?;
        Some((first_value, second_value))
    }

    #[inline]
    fn second_at(&self, index: usize) -> u16 {
        self.data
            .u16_at(2 + index * self.record_size)
            .unwrap_or(u16::MAX)
    }
}

impl<'a> ClassPairPos<'a> {
    #[inline]
    pub fn coverage(&self) -> Coverage<'a> {
        Coverage::from_offset(self.data, 2)
    }

    #[inline]
    pub fn first_class_def(&self) -> ClassDef<'a> {
        ClassDef::from_offset(self.data, 8)
    }

    #[inline]
    pub fn second_class_def(&self) -> ClassDef<'a> {
        ClassDef::from_offset(self.data, 10)
    }

    #[inline]
    pub fn first_class_count(&self) -> u16 {
        self.first_class_count
    }

    #[inline]
    pub fn second_class_count(&self) -> u16 {
        self.second_class_count
    }

    #[inline]
    pub fn class_values(
        &self,
        first_class: u16,
        second_class: u16,
    ) -> Option<(ValueRecord<'a>, ValueRecord<'a>)> {
        if first_class >= self.first_class_count || second_class >= self.second_class_count {
            return None;
        }
        let first_size = self.formats.0.size();
        let record_size = first_size + self.formats.1.size();
        let index = usize::from(first_class) * usize::from(self.second_class_count)
            + usize::from(second_class);
        let record = 16 + index * record_size;
        Some((
            ValueRecord::new(self.formats.0, self.data, record)?,
            ValueRecord::new(self.formats.1, self.data, record + first_size)?,
        ))
    }
}
