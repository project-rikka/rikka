use crate::parse::array::{Array, Scalar};
use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;

#[derive(Clone, Copy, Debug, Default)]
pub struct ClassDef<'a> {
    kind: ClassDefKind<'a>,
}

#[derive(Clone, Copy, Debug, Default)]
enum ClassDefKind<'a> {
    #[default]
    Empty,
    Glyphs {
        start: u16,
        classes: Array<'a, u16>,
    },
    Ranges(Array<'a, ClassRange>),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ClassRange {
    start: u16,
    end: u16,
    class: u16,
}

impl Scalar for ClassRange {
    const SIZE: usize = 6;

    #[inline]
    fn parse(bytes: &[u8]) -> Option<Self> {
        Some(Self {
            start: bytes.u16_at(0)?,
            end: bytes.u16_at(2)?,
            class: bytes.u16_at(4)?,
        })
    }
}

impl<'a> ClassDef<'a> {
    #[inline]
    pub fn new(data: &'a [u8]) -> Option<Self> {
        let kind = match data.u16_at(0)? {
            1 => {
                let start = data.u16_at(2)?;
                let count = usize::from(data.u16_at(4)?);
                ClassDefKind::Glyphs {
                    start,
                    classes: data.array(6, count)?,
                }
            }
            2 => {
                let count = usize::from(data.u16_at(2)?);
                ClassDefKind::Ranges(data.array(4, count)?)
            }
            _ => return None,
        };
        Some(Self { kind })
    }

    #[inline]
    pub(crate) fn from_offset(data: &'a [u8], offset_at: usize) -> Self {
        Self::or_empty(data.offset16(0, offset_at))
    }

    #[inline]
    pub(crate) fn or_empty(data: Option<&'a [u8]>) -> Self {
        data.and_then(Self::new).unwrap_or_default()
    }

    #[inline]
    pub fn empty() -> Self {
        Self::default()
    }

    #[inline]
    pub fn get(&self, glyph: GlyphId) -> u16 {
        match self.kind {
            ClassDefKind::Empty => 0,
            ClassDefKind::Glyphs { start, classes } => glyph
                .0
                .checked_sub(start)
                .and_then(|index| classes.get(usize::from(index)))
                .unwrap_or(0),
            ClassDefKind::Ranges(ranges) => ranges
                .binary_search_by(|range| {
                    if range.end < glyph.0 {
                        std::cmp::Ordering::Less
                    } else if range.start > glyph.0 {
                        std::cmp::Ordering::Greater
                    } else {
                        std::cmp::Ordering::Equal
                    }
                })
                .ok()
                .and_then(|index| ranges.get(index))
                .map_or(0, |range| range.class),
        }
    }

    #[inline]
    pub fn ranges(&self) -> impl Iterator<Item = (GlyphId, GlyphId, u16)> + use<'a> {
        let (start, classes, ranges) = match self.kind {
            ClassDefKind::Empty => (0, Array::empty(), Array::empty()),
            ClassDefKind::Glyphs { start, classes } => (start, classes, Array::empty()),
            ClassDefKind::Ranges(ranges) => (0, Array::empty(), ranges),
        };
        classes
            .iter()
            .enumerate()
            .filter_map(move |(index, class)| {
                let glyph = GlyphId(u16::try_from(usize::from(start) + index).ok()?);
                Some((glyph, glyph, class))
            })
            .chain(
                ranges
                    .iter()
                    .filter(|range| range.start <= range.end)
                    .map(|range| (GlyphId(range.start), GlyphId(range.end), range.class)),
            )
    }
}
