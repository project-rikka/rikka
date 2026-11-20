use crate::color::bitmap_glyph::{BitmapGlyph, BitmapMetrics, ImageLocation, decode_image};
use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::sfnt::table_id::TableId;

const BITMAP_SIZE_RECORD: usize = 48;
const INDEX_RECORD_SIZE: usize = 8;

#[derive(Clone, Copy, Debug)]
pub struct BitmapStrikes<'a> {
    location: &'a [u8],
    images: &'a [u8],
    count: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct BitmapStrike<'a> {
    location: &'a [u8],
    images: &'a [u8],
    record: &'a [u8],
}

impl<'a> BitmapStrikes<'a> {
    pub fn new(location: &'a [u8], images: &'a [u8]) -> Option<Self> {
        let major = location.u16_at(0)?;
        if major != 2 && major != 3 {
            return None;
        }
        let count = location.u32_at(4)? as usize;
        location.range(8, count.checked_mul(BITMAP_SIZE_RECORD)?)?;
        Some(Self {
            location,
            images,
            count,
        })
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.count
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn get(&self, index: usize) -> Option<BitmapStrike<'a>> {
        if index >= self.count {
            return None;
        }
        Some(BitmapStrike {
            location: self.location,
            images: self.images,
            record: self
                .location
                .range(8 + index * BITMAP_SIZE_RECORD, BITMAP_SIZE_RECORD)?,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = BitmapStrike<'a>> + use<'a> {
        let strikes = *self;
        (0..self.count).filter_map(move |index| strikes.get(index))
    }

    pub fn glyph(&self, glyph: GlyphId, ppem: u16) -> Option<BitmapGlyph<'a>> {
        let mut best: Option<BitmapGlyph<'a>> = None;
        for strike in self.iter() {
            let Some(candidate) = strike.glyph(glyph) else {
                continue;
            };
            let better = match &best {
                None => true,
                Some(current) => {
                    prefers(u16::from(candidate.ppem_y), u16::from(current.ppem_y), ppem)
                }
            };
            if better {
                best = Some(candidate);
            }
        }
        best
    }
}

pub(crate) fn prefers(candidate: u16, current: u16, requested: u16) -> bool {
    match (candidate >= requested, current >= requested) {
        (true, true) => candidate < current,
        (true, false) => true,
        (false, true) => false,
        (false, false) => candidate > current,
    }
}

impl<'a> BitmapStrike<'a> {
    #[inline]
    pub fn ppem_x(&self) -> u8 {
        self.record.u8_at(44).unwrap_or(0)
    }

    #[inline]
    pub fn ppem_y(&self) -> u8 {
        self.record.u8_at(45).unwrap_or(0)
    }

    #[inline]
    pub fn bit_depth(&self) -> u8 {
        self.record.u8_at(46).unwrap_or(0)
    }

    #[inline]
    pub fn first_glyph(&self) -> GlyphId {
        GlyphId(self.record.u16_at(40).unwrap_or(0))
    }

    #[inline]
    pub fn last_glyph(&self) -> GlyphId {
        GlyphId(self.record.u16_at(42).unwrap_or(0))
    }

    pub fn glyph(&self, glyph: GlyphId) -> Option<BitmapGlyph<'a>> {
        let image = self.locate(glyph)?;
        let (metrics, data) = decode_image(image, self.bit_depth())?;
        Some(BitmapGlyph {
            ppem_x: self.ppem_x(),
            ppem_y: self.ppem_y(),
            bit_depth: self.bit_depth(),
            metrics,
            data,
        })
    }

    fn locate(&self, glyph: GlyphId) -> Option<ImageLocation<'a>> {
        if glyph < self.first_glyph() || glyph > self.last_glyph() {
            return None;
        }
        let list_offset = self.record.u32_at(0)? as usize;
        let list_count = self.record.u32_at(8)? as usize;
        let list = self.location.tail(list_offset)?;
        list.range(0, list_count.checked_mul(INDEX_RECORD_SIZE)?)?;
        (0..list_count).find_map(|index| {
            let record = index * INDEX_RECORD_SIZE;
            let first = list.u16_at(record)?;
            let last = list.u16_at(record + 2)?;
            if glyph.0 < first || glyph.0 > last {
                return None;
            }
            let subtable = list.tail(list.u32_at(record + 4)? as usize)?;
            self.locate_in_subtable(subtable, glyph.0 - first, glyph)
        })
    }

    fn locate_in_subtable(
        &self,
        subtable: &'a [u8],
        index: u16,
        glyph: GlyphId,
    ) -> Option<ImageLocation<'a>> {
        let index_format = subtable.u16_at(0)?;
        let format = subtable.u16_at(2)?;
        let image_base = subtable.u32_at(4)? as usize;
        let index = usize::from(index);
        let (start, end, metrics) = match index_format {
            1 => (
                subtable.u32_at(8 + index * 4)? as usize,
                subtable.u32_at(12 + index * 4)? as usize,
                None,
            ),
            3 => (
                usize::from(subtable.u16_at(8 + index * 2)?),
                usize::from(subtable.u16_at(10 + index * 2)?),
                None,
            ),
            2 => {
                let size = subtable.u32_at(8)? as usize;
                let start = index.checked_mul(size)?;
                (
                    start,
                    start.checked_add(size)?,
                    Some(BitmapMetrics::big(subtable.tail(12)?)?),
                )
            }
            4 => {
                let count = subtable.u32_at(8)? as usize;
                let pairs = subtable.tail(12)?;
                pairs.range(0, count.checked_add(1)?.checked_mul(4)?)?;
                let position = search_glyphs(count, |at| pairs.u16_at(at * 4), glyph.0)?;
                (
                    usize::from(pairs.u16_at(position * 4 + 2)?),
                    usize::from(pairs.u16_at(position * 4 + 6)?),
                    None,
                )
            }
            5 => {
                let size = subtable.u32_at(8)? as usize;
                let count = subtable.u32_at(20)? as usize;
                let glyphs = subtable.tail(24)?;
                let position = search_glyphs(count, |at| glyphs.u16_at(at * 2), glyph.0)?;
                let start = position.checked_mul(size)?;
                (
                    start,
                    start.checked_add(size)?,
                    Some(BitmapMetrics::big(subtable.tail(12)?)?),
                )
            }
            _ => return None,
        };
        if end <= start {
            return None;
        }
        let data = self
            .images
            .range(image_base.checked_add(start)?, end - start)?;
        Some(ImageLocation {
            format,
            data,
            metrics,
        })
    }
}

fn search_glyphs(
    count: usize,
    glyph_at: impl Fn(usize) -> Option<u16>,
    glyph: u16,
) -> Option<usize> {
    let (mut low, mut high) = (0, count);
    while low < high {
        let middle = low + (high - low) / 2;
        let found = glyph_at(middle)?;
        if found < glyph {
            low = middle + 1;
        } else if found > glyph {
            high = middle;
        } else {
            return Some(middle);
        }
    }
    None
}

impl<'a> Font<'a> {
    pub fn bitmap_strikes(&self) -> Option<BitmapStrikes<'a>> {
        let color = self
            .table_data(TableId::Cblc)
            .zip(self.table_data(TableId::Cbdt))
            .and_then(|(location, images)| BitmapStrikes::new(location, images));
        color.or_else(|| {
            let (location, images) = self
                .table_data(TableId::Eblc)
                .zip(self.table_data(TableId::Ebdt))?;
            BitmapStrikes::new(location, images)
        })
    }

    pub fn bitmap_glyph(&self, glyph: GlyphId, ppem: u16) -> Option<BitmapGlyph<'a>> {
        self.bitmap_strikes()?.glyph(glyph, ppem)
    }
}
