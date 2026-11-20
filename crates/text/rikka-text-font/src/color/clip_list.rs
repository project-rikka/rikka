use std::cmp::Ordering;

use crate::color::colr_variations::ColrVariations;
use crate::outline::bounds::Bounds;
use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;

const CLIP_RECORD_SIZE: usize = 7;
const NO_VARIATION: u32 = u32::MAX;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ClipList<'a> {
    data: &'a [u8],
    count: usize,
}

impl<'a> ClipList<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        if data.u8_at(0)? != 1 {
            return None;
        }
        let count = data.u32_at(1)? as usize;
        data.range(5, count.checked_mul(CLIP_RECORD_SIZE)?)?;
        Some(Self { data, count })
    }

    fn record(&self, index: usize) -> Option<(u16, u16, u32)> {
        let at = 5 + index * CLIP_RECORD_SIZE;
        Some((
            self.data.u16_at(at)?,
            self.data.u16_at(at + 2)?,
            self.data.u24_at(at + 4)?,
        ))
    }

    pub(crate) fn clip_box(
        &self,
        glyph: GlyphId,
        variations: &ColrVariations<'_>,
    ) -> Option<Bounds> {
        let (mut low, mut high) = (0, self.count);
        while low < high {
            let middle = low + (high - low) / 2;
            let (start, end, offset) = self.record(middle)?;
            let order = if glyph.0 < start {
                Ordering::Greater
            } else if glyph.0 > end {
                Ordering::Less
            } else {
                Ordering::Equal
            };
            match order {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return self.resolve(offset, variations),
            }
        }
        None
    }

    fn resolve(&self, offset: u32, variations: &ColrVariations<'_>) -> Option<Bounds> {
        let clip = self.data.tail(offset as usize)?;
        let base = match clip.u8_at(0)? {
            1 => NO_VARIATION,
            2 => clip.u32_at(9)?,
            _ => return None,
        };
        Some(Bounds {
            x_min: variations.fword(clip.i16_at(1)?, base, 0),
            y_min: variations.fword(clip.i16_at(3)?, base, 1),
            x_max: variations.fword(clip.i16_at(5)?, base, 2),
            y_max: variations.fword(clip.i16_at(7)?, base, 3),
        })
    }
}
