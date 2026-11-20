use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::table_id::TableId;

#[derive(Clone, Copy, Debug)]
pub struct MetricsHeader<'a> {
    data: &'a [u8],
}

impl<'a> MetricsHeader<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        data.range(0, 36)?;
        Some(Self { data })
    }

    #[inline]
    fn i16(&self, offset: usize) -> i16 {
        self.data.i16_at(offset).unwrap_or(0)
    }

    pub fn ascender(&self) -> i16 {
        self.i16(4)
    }

    pub fn descender(&self) -> i16 {
        self.i16(6)
    }

    pub fn line_gap(&self) -> i16 {
        self.i16(8)
    }

    pub fn advance_max(&self) -> u16 {
        self.data.u16_at(10).unwrap_or(0)
    }

    pub fn min_leading_bearing(&self) -> i16 {
        self.i16(12)
    }

    pub fn min_trailing_bearing(&self) -> i16 {
        self.i16(14)
    }

    pub fn max_extent(&self) -> i16 {
        self.i16(16)
    }

    pub fn caret_slope_rise(&self) -> i16 {
        self.i16(18)
    }

    pub fn caret_slope_run(&self) -> i16 {
        self.i16(20)
    }

    pub fn caret_offset(&self) -> i16 {
        self.i16(22)
    }

    pub fn long_metric_count(&self) -> u16 {
        self.data.u16_at(34).unwrap_or(0)
    }
}

impl<'a> Font<'a> {
    pub fn hhea(&self) -> Option<MetricsHeader<'a>> {
        MetricsHeader::new(self.table_data(TableId::Hhea)?)
    }

    pub fn vhea(&self) -> Option<MetricsHeader<'a>> {
        MetricsHeader::new(self.table_data(TableId::Vhea)?)
    }
}
