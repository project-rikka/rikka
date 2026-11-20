use crate::layout::coverage::Coverage;
use crate::layout::gpos::value_record::{ValueFormat, ValueRecord};
use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;

#[derive(Clone, Copy, Debug)]
pub struct SinglePos<'a> {
    data: &'a [u8],
    format: ValueFormat,
    count: Option<u16>,
}

impl<'a> SinglePos<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        let format = ValueFormat(data.u16_at(4)?);
        let count = match data.u16_at(0)? {
            1 => None,
            2 => Some(data.u16_at(6)?),
            _ => return None,
        };
        Some(Self {
            data,
            format,
            count,
        })
    }

    #[inline]
    pub fn coverage(&self) -> Coverage<'a> {
        Coverage::from_offset(self.data, 2)
    }

    #[inline]
    pub fn value_format(&self) -> ValueFormat {
        self.format
    }

    #[inline]
    pub fn value_at(&self, coverage_index: u16) -> Option<ValueRecord<'a>> {
        match self.count {
            None => ValueRecord::new(self.format, self.data, 6),
            Some(count) if coverage_index < count => {
                let offset = 8 + usize::from(coverage_index) * self.format.size();
                ValueRecord::new(self.format, self.data, offset)
            }
            Some(_) => None,
        }
    }

    #[inline]
    pub fn value(&self, glyph: GlyphId) -> Option<ValueRecord<'a>> {
        self.value_at(self.coverage().get(glyph)?)
    }
}
