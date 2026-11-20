use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;

#[derive(Clone, Copy, Debug)]
pub(crate) enum FdSelect<'a> {
    Format0(&'a [u8]),
    Format3 { ranges: &'a [u8], count: usize },
    Format4 { ranges: &'a [u8], count: usize },
}

impl<'a> FdSelect<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        match data.u8_at(0)? {
            0 => Some(Self::Format0(data.tail(1)?)),
            3 => {
                let count = usize::from(data.u16_at(1)?);
                Some(Self::Format3 {
                    ranges: data.range(3, count * 3 + 2)?,
                    count,
                })
            }
            4 => {
                let count = data.u32_at(1)? as usize;
                Some(Self::Format4 {
                    ranges: data.range(5, count.checked_mul(6)?.checked_add(4)?)?,
                    count,
                })
            }
            _ => None,
        }
    }

    pub(crate) fn font_dict(&self, glyph: GlyphId) -> Option<u16> {
        let glyph = u32::from(glyph.0);
        match *self {
            Self::Format0(data) => data.u8_at(glyph as usize).map(u16::from),
            Self::Format3 { ranges, count } => search(count, glyph, |index| {
                let at = index * 3;
                Some((
                    u32::from(ranges.u16_at(at)?),
                    u16::from(ranges.u8_at(at + 2)?),
                    u32::from(ranges.u16_at(at + 3)?),
                ))
            }),
            Self::Format4 { ranges, count } => search(count, glyph, |index| {
                let at = index * 6;
                Some((
                    ranges.u32_at(at)?,
                    ranges.u16_at(at + 4)?,
                    ranges.u32_at(at + 6)?,
                ))
            }),
        }
    }
}

fn search(
    count: usize,
    glyph: u32,
    range: impl Fn(usize) -> Option<(u32, u16, u32)>,
) -> Option<u16> {
    let mut low = 0;
    let mut high = count;
    while low < high {
        let middle = low + (high - low) / 2;
        let (first, font_dict, end) = range(middle)?;
        if glyph < first {
            high = middle;
        } else if glyph >= end {
            low = middle + 1;
        } else {
            return Some(font_dict);
        }
    }
    None
}
