use crate::layout::coverage::Coverage;
use crate::parse::array::Array;
use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;

#[derive(Clone, Copy, Debug)]
pub struct SingleSubst<'a> {
    data: &'a [u8],
    kind: SingleSubstKind<'a>,
}

#[derive(Clone, Copy, Debug)]
enum SingleSubstKind<'a> {
    Delta(u16),
    Glyphs(Array<'a, GlyphId>),
}

impl<'a> SingleSubst<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        let kind = match data.u16_at(0)? {
            1 => SingleSubstKind::Delta(data.u16_at(4)?),
            2 => {
                let count = usize::from(data.u16_at(4)?);
                SingleSubstKind::Glyphs(data.array(6, count)?)
            }
            _ => return None,
        };
        Some(Self { data, kind })
    }

    #[inline]
    pub fn coverage(&self) -> Coverage<'a> {
        Coverage::from_offset(self.data, 2)
    }

    #[inline]
    pub fn substitute(&self, glyph: GlyphId) -> Option<GlyphId> {
        self.substitute_at(self.coverage().get(glyph)?, glyph)
    }

    #[inline]
    pub fn substitute_at(&self, index: u16, glyph: GlyphId) -> Option<GlyphId> {
        match self.kind {
            SingleSubstKind::Delta(delta) => Some(GlyphId(glyph.0.wrapping_add(delta))),
            SingleSubstKind::Glyphs(glyphs) => glyphs.get(usize::from(index)),
        }
    }
}
