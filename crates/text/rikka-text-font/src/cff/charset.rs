use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;

const ISO_ADOBE_LAST_SID: u16 = 228;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Charset<'a> {
    IsoAdobe,
    Predefined,
    Format0(&'a [u8]),
    Format1(&'a [u8]),
    Format2(&'a [u8]),
}

impl<'a> Charset<'a> {
    pub(crate) fn new(cff: &'a [u8], offset: usize) -> Option<Self> {
        match offset {
            0 => Some(Self::IsoAdobe),
            1 | 2 => Some(Self::Predefined),
            _ => {
                let data = cff.tail(offset)?;
                let body = data.tail(1)?;
                match data.u8_at(0)? {
                    0 => Some(Self::Format0(body)),
                    1 => Some(Self::Format1(body)),
                    2 => Some(Self::Format2(body)),
                    _ => None,
                }
            }
        }
    }

    pub(crate) fn sid(&self, glyph: GlyphId, glyph_count: u32) -> Option<u16> {
        if u32::from(glyph.0) >= glyph_count {
            return None;
        }
        if glyph.0 == 0 {
            return Some(0);
        }
        match *self {
            Self::IsoAdobe => (glyph.0 <= ISO_ADOBE_LAST_SID).then_some(glyph.0),
            Self::Predefined => None,
            Self::Format0(data) => data.u16_at(usize::from(glyph.0 - 1) * 2),
            Self::Format1(data) => ranged_sid(data, 3, glyph.0, glyph_count),
            Self::Format2(data) => ranged_sid(data, 4, glyph.0, glyph_count),
        }
    }

    pub(crate) fn glyph(&self, sid: u16, glyph_count: u32) -> Option<GlyphId> {
        if sid == 0 {
            return Some(GlyphId(0));
        }
        let found = match *self {
            Self::IsoAdobe => (sid <= ISO_ADOBE_LAST_SID).then_some(sid),
            Self::Predefined => None,
            Self::Format0(data) => (1..glyph_count.min(65536))
                .find(|&gid| data.u16_at((gid as usize - 1) * 2) == Some(sid))
                .map(|gid| gid as u16),
            Self::Format1(data) => ranged_glyph(data, 3, sid, glyph_count),
            Self::Format2(data) => ranged_glyph(data, 4, sid, glyph_count),
        }?;
        (u32::from(found) < glyph_count).then_some(GlyphId(found))
    }
}

fn range_at(data: &[u8], record_size: usize, index: usize) -> Option<(u16, u16)> {
    let at = index * record_size;
    let first = data.u16_at(at)?;
    let left = if record_size == 3 {
        u16::from(data.u8_at(at + 2)?)
    } else {
        data.u16_at(at + 2)?
    };
    Some((first, left))
}

fn ranged_sid(data: &[u8], record_size: usize, glyph: u16, glyph_count: u32) -> Option<u16> {
    let mut next_glyph = 1u32;
    let mut index = 0;
    while next_glyph < glyph_count {
        let (first, left) = range_at(data, record_size, index)?;
        let covered = u32::from(left) + 1;
        if u32::from(glyph) < next_glyph + covered {
            return first.checked_add((u32::from(glyph) - next_glyph) as u16);
        }
        next_glyph += covered;
        index += 1;
    }
    None
}

fn ranged_glyph(data: &[u8], record_size: usize, sid: u16, glyph_count: u32) -> Option<u16> {
    let mut next_glyph = 1u32;
    let mut index = 0;
    while next_glyph < glyph_count {
        let (first, left) = range_at(data, record_size, index)?;
        if sid >= first && u32::from(sid) <= u32::from(first) + u32::from(left) {
            let glyph = next_glyph + u32::from(sid - first);
            return u16::try_from(glyph).ok();
        }
        next_glyph += u32::from(left) + 1;
        index += 1;
    }
    None
}
