use crate::parse::array::{Array, Scalar};
use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;

#[derive(Clone, Copy, Debug)]
pub struct Coverage<'a> {
    kind: CoverageKind<'a>,
}

#[derive(Clone, Copy, Debug)]
enum CoverageKind<'a> {
    Glyphs(Array<'a, GlyphId>),
    Ranges(Array<'a, RangeRecord>),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RangeRecord {
    start: u16,
    end: u16,
    value: u16,
}

impl Scalar for RangeRecord {
    const SIZE: usize = 6;

    #[inline]
    fn parse(bytes: &[u8]) -> Option<Self> {
        Some(Self {
            start: bytes.u16_at(0)?,
            end: bytes.u16_at(2)?,
            value: bytes.u16_at(4)?,
        })
    }
}

impl Default for Coverage<'_> {
    fn default() -> Self {
        Self {
            kind: CoverageKind::Glyphs(Array::empty()),
        }
    }
}

impl<'a> Coverage<'a> {
    #[inline]
    pub(crate) fn from_offset(data: &'a [u8], offset_at: usize) -> Self {
        data.offset16(0, offset_at)
            .and_then(Self::new)
            .unwrap_or_default()
    }

    #[inline]
    pub fn new(data: &'a [u8]) -> Option<Self> {
        let count = usize::from(data.u16_at(2)?);
        let kind = match data.u16_at(0)? {
            1 => CoverageKind::Glyphs(data.array(4, count)?),
            2 => CoverageKind::Ranges(data.array(4, count)?),
            _ => return None,
        };
        Some(Self { kind })
    }

    #[inline]
    pub fn get(&self, glyph: GlyphId) -> Option<u16> {
        match self.kind {
            CoverageKind::Glyphs(glyphs) => glyphs
                .binary_search_by(|probe| probe.cmp(&glyph))
                .ok()
                .map(|index| index as u16),
            CoverageKind::Ranges(ranges) => {
                let index = ranges
                    .binary_search_by(|range| {
                        if range.end < glyph.0 {
                            std::cmp::Ordering::Less
                        } else if range.start > glyph.0 {
                            std::cmp::Ordering::Greater
                        } else {
                            std::cmp::Ordering::Equal
                        }
                    })
                    .ok()?;
                let range = ranges.get(index)?;
                Some(range.value.wrapping_add(glyph.0 - range.start))
            }
        }
    }

    pub fn is_ordered(&self) -> bool {
        match self.kind {
            CoverageKind::Glyphs(glyphs) => glyphs
                .iter()
                .zip(glyphs.iter().skip(1))
                .all(|(previous, next)| previous < next),
            CoverageKind::Ranges(ranges) => {
                ranges.iter().all(|range| range.start <= range.end)
                    && ranges
                        .iter()
                        .zip(ranges.iter().skip(1))
                        .all(|(previous, next)| previous.end < next.start)
            }
        }
    }

    #[inline]
    pub fn contains(&self, glyph: GlyphId) -> bool {
        self.get(glyph).is_some()
    }

    #[inline]
    pub fn ranges(&self) -> impl Iterator<Item = (GlyphId, GlyphId)> + use<'a> {
        let (glyphs, ranges) = match self.kind {
            CoverageKind::Glyphs(glyphs) => (glyphs, Array::empty()),
            CoverageKind::Ranges(ranges) => (Array::empty(), ranges),
        };
        glyphs.iter().map(|glyph| (glyph, glyph)).chain(
            ranges
                .iter()
                .filter(|range| range.start <= range.end)
                .map(|range| (GlyphId(range.start), GlyphId(range.end))),
        )
    }

    #[inline]
    pub fn entries(&self) -> impl Iterator<Item = (GlyphId, u16)> + use<'a> {
        let (glyphs, ranges) = match self.kind {
            CoverageKind::Glyphs(glyphs) => (glyphs, Array::empty()),
            CoverageKind::Ranges(ranges) => (Array::empty(), ranges),
        };
        glyphs
            .iter()
            .enumerate()
            .map(|(index, glyph)| (glyph, index as u16))
            .chain(ranges.iter().flat_map(|range: RangeRecord| {
                (range.start..=range.end).map(move |glyph| {
                    (
                        GlyphId(glyph),
                        range.value.wrapping_add(glyph - range.start),
                    )
                })
            }))
    }

    #[inline]
    pub fn glyphs(&self) -> impl Iterator<Item = GlyphId> + use<'a> {
        self.ranges()
            .flat_map(|(first, last)| (first.0..=last.0).map(GlyphId))
    }
}
