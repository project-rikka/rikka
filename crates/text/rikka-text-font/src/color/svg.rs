use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::sfnt::table_id::TableId;

const RECORD_SIZE: usize = 12;
const GZIP_MAGIC: [u8; 2] = [0x1F, 0x8B];

#[derive(Clone, Copy, Debug)]
pub struct Svg<'a> {
    list: &'a [u8],
    count: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct SvgDocument<'a> {
    pub data: &'a [u8],
    pub first_glyph: GlyphId,
    pub last_glyph: GlyphId,
}

impl SvgDocument<'_> {
    #[inline]
    pub fn is_compressed(&self) -> bool {
        self.data.starts_with(&GZIP_MAGIC)
    }
}

impl<'a> Svg<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        if data.u16_at(0)? != 0 {
            return None;
        }
        let list = data.offset32(0, 2)?;
        let count = usize::from(list.u16_at(0)?);
        list.range(2, count * RECORD_SIZE)?;
        Some(Self { list, count })
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.count
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn get(&self, index: usize) -> Option<SvgDocument<'a>> {
        if index >= self.count {
            return None;
        }
        let record = 2 + index * RECORD_SIZE;
        let offset = self.list.u32_at(record + 4)? as usize;
        let len = self.list.u32_at(record + 8)? as usize;
        Some(SvgDocument {
            data: self.list.range(offset, len)?,
            first_glyph: GlyphId(self.list.u16_at(record)?),
            last_glyph: GlyphId(self.list.u16_at(record + 2)?),
        })
    }

    pub fn document(&self, glyph: GlyphId) -> Option<SvgDocument<'a>> {
        let (mut low, mut high) = (0, self.count);
        while low < high {
            let middle = low + (high - low) / 2;
            let record = 2 + middle * RECORD_SIZE;
            let first = self.list.u16_at(record)?;
            let last = self.list.u16_at(record + 2)?;
            if glyph.0 < first {
                high = middle;
            } else if glyph.0 > last {
                low = middle + 1;
            } else {
                return self.get(middle);
            }
        }
        None
    }
}

impl<'a> Font<'a> {
    pub fn svg(&self) -> Option<Svg<'a>> {
        Svg::new(self.table_data(TableId::Svg)?)
    }

    pub fn svg_document(&self, glyph: GlyphId) -> Option<SvgDocument<'a>> {
        self.svg()?.document(glyph)
    }
}
