use crate::color::bitmap_strikes::prefers;
use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::sfnt::table_id::TableId;
use crate::sfnt::tag::Tag;

const DRAW_OUTLINES: u16 = 0x0002;
const GLYPH_HEADER_SIZE: usize = 8;
const DUPE: Tag = Tag::new(b"dupe");

#[derive(Clone, Copy, Debug)]
pub struct Sbix<'a> {
    data: &'a [u8],
    flags: u16,
    strike_count: usize,
    glyph_count: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct SbixStrike<'a> {
    data: &'a [u8],
    glyph_count: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SbixFormat {
    Png,
    Jpeg,
    Tiff,
    Mask,
    Other(Tag),
}

#[derive(Clone, Copy, Debug)]
pub struct SbixGlyph<'a> {
    pub ppem: u16,
    pub ppi: u16,
    pub origin_x: i16,
    pub origin_y: i16,
    pub format: SbixFormat,
    pub data: &'a [u8],
}

impl SbixFormat {
    fn from_tag(tag: Tag) -> Self {
        match &tag.to_bytes() {
            b"png " => Self::Png,
            b"jpg " => Self::Jpeg,
            b"tiff" => Self::Tiff,
            b"mask" => Self::Mask,
            _ => Self::Other(tag),
        }
    }
}

impl<'a> Sbix<'a> {
    pub fn new(data: &'a [u8], glyph_count: u16) -> Option<Self> {
        if data.u16_at(0)? != 1 {
            return None;
        }
        let flags = data.u16_at(2)?;
        let strike_count = data.u32_at(4)? as usize;
        data.range(8, strike_count.checked_mul(4)?)?;
        Some(Self {
            data,
            flags,
            strike_count,
            glyph_count,
        })
    }

    #[inline]
    pub fn draws_outlines(&self) -> bool {
        self.flags & DRAW_OUTLINES != 0
    }

    #[inline]
    pub fn strike_count(&self) -> usize {
        self.strike_count
    }

    pub fn strike(&self, index: usize) -> Option<SbixStrike<'a>> {
        if index >= self.strike_count {
            return None;
        }
        let data = self.data.offset32(0, 8 + index * 4)?;
        data.range(4, (usize::from(self.glyph_count) + 1) * 4)?;
        Some(SbixStrike {
            data,
            glyph_count: self.glyph_count,
        })
    }

    pub fn strikes(&self) -> impl Iterator<Item = SbixStrike<'a>> + use<'a> {
        let sbix = *self;
        (0..self.strike_count).filter_map(move |index| sbix.strike(index))
    }

    pub fn glyph(&self, glyph: GlyphId, ppem: u16) -> Option<SbixGlyph<'a>> {
        let mut best: Option<SbixGlyph<'a>> = None;
        for strike in self.strikes() {
            let Some(candidate) = strike.glyph(glyph) else {
                continue;
            };
            if best.is_none_or(|current| prefers(candidate.ppem, current.ppem, ppem)) {
                best = Some(candidate);
            }
        }
        best
    }
}

impl<'a> SbixStrike<'a> {
    #[inline]
    pub fn ppem(&self) -> u16 {
        self.data.u16_at(0).unwrap_or(0)
    }

    #[inline]
    pub fn ppi(&self) -> u16 {
        self.data.u16_at(2).unwrap_or(0)
    }

    pub fn glyph(&self, glyph: GlyphId) -> Option<SbixGlyph<'a>> {
        let record = self.record(glyph)?;
        let graphic = record.tag_at(4)?;
        let record = if graphic == DUPE {
            let target = self.record(GlyphId(record.u16_at(GLYPH_HEADER_SIZE)?))?;
            if target.tag_at(4)? == DUPE {
                return None;
            }
            target
        } else {
            record
        };
        Some(SbixGlyph {
            ppem: self.ppem(),
            ppi: self.ppi(),
            origin_x: record.i16_at(0)?,
            origin_y: record.i16_at(2)?,
            format: SbixFormat::from_tag(record.tag_at(4)?),
            data: record.tail(GLYPH_HEADER_SIZE)?,
        })
    }

    fn record(&self, glyph: GlyphId) -> Option<&'a [u8]> {
        if glyph.0 >= self.glyph_count {
            return None;
        }
        let at = 4 + glyph.index() * 4;
        let start = self.data.u32_at(at)? as usize;
        let end = self.data.u32_at(at + 4)? as usize;
        if end < start.checked_add(GLYPH_HEADER_SIZE)? {
            return None;
        }
        self.data.range(start, end - start)
    }
}

impl<'a> Font<'a> {
    pub fn sbix(&self) -> Option<Sbix<'a>> {
        Sbix::new(self.table_data(TableId::Sbix)?, self.glyph_count())
    }
}
