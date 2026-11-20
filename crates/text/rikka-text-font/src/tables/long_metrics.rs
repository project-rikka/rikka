use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::sfnt::table_id::TableId;

#[derive(Clone, Copy, Debug)]
pub struct LongMetrics<'a> {
    data: &'a [u8],
    long_count: usize,
    glyph_count: usize,
}

impl<'a> LongMetrics<'a> {
    pub fn new(data: &'a [u8], long_count: u16, glyph_count: u16) -> Option<Self> {
        let long_count = usize::from(long_count).min(data.len() / 4);
        if long_count == 0 {
            return None;
        }
        Some(Self {
            data,
            long_count,
            glyph_count: usize::from(glyph_count),
        })
    }

    #[inline]
    pub fn advance(&self, glyph: GlyphId) -> Option<u16> {
        let index = glyph.index();
        if index >= self.glyph_count.max(self.long_count) {
            return None;
        }
        self.data.u16_at(index.min(self.long_count - 1) * 4)
    }

    #[inline]
    pub fn side_bearing(&self, glyph: GlyphId) -> Option<i16> {
        let index = glyph.index();
        if index < self.long_count {
            return self.data.i16_at(index * 4 + 2);
        }
        if index >= self.glyph_count {
            return None;
        }
        self.data
            .i16_at(self.long_count * 4 + (index - self.long_count) * 2)
    }
}

impl<'a> Font<'a> {
    pub fn hmtx(&self) -> Option<LongMetrics<'a>> {
        let header = self.hhea()?;
        LongMetrics::new(
            self.table_data(TableId::Hmtx)?,
            header.long_metric_count(),
            self.glyph_count(),
        )
    }

    pub fn vmtx(&self) -> Option<LongMetrics<'a>> {
        let header = self.vhea()?;
        LongMetrics::new(
            self.table_data(TableId::Vmtx)?,
            header.long_metric_count(),
            self.glyph_count(),
        )
    }
}
