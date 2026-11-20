use crate::cmap::format4::Format4;
use crate::cmap::groups::{GroupMapping, Groups};
use crate::parse::array::Array;
use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;

#[derive(Clone, Copy, Debug)]
pub struct CmapSubtable<'a> {
    pub platform_id: u16,
    pub encoding_id: u16,
    pub format: u16,
    kind: Kind<'a>,
}

#[derive(Clone, Copy, Debug)]
enum Kind<'a> {
    ByteEncoding(&'a [u8]),
    SegmentDelta(Format4<'a>),
    Trimmed { first: u32, glyphs: Array<'a, u16> },
    Groups(Groups<'a>),
    Other,
}

impl<'a> CmapSubtable<'a> {
    pub(crate) fn new(platform_id: u16, encoding_id: u16, data: &'a [u8]) -> Option<Self> {
        let format = data.u16_at(0)?;
        let kind = match format {
            0 => Kind::ByteEncoding(data.range(6, 256)?),
            4 => Kind::SegmentDelta(Format4::new(data)?),
            6 => {
                let first = u32::from(data.u16_at(6)?);
                let count = usize::from(data.u16_at(8)?);
                Kind::Trimmed {
                    first,
                    glyphs: data.array(10, count)?,
                }
            }
            10 => {
                let first = data.u32_at(12)?;
                let count = data.u32_at(16)? as usize;
                Kind::Trimmed {
                    first,
                    glyphs: data.array(20, count)?,
                }
            }
            12 => Kind::Groups(Groups::new(data, GroupMapping::Sequential)?),
            13 => Kind::Groups(Groups::new(data, GroupMapping::Constant)?),
            _ => Kind::Other,
        };
        Some(Self {
            platform_id,
            encoding_id,
            format,
            kind,
        })
    }

    pub(crate) fn is_supported(&self) -> bool {
        !matches!(self.kind, Kind::Other)
    }

    #[inline]
    pub fn map(&self, codepoint: u32) -> Option<GlyphId> {
        let glyph = match self.kind {
            Kind::ByteEncoding(glyphs) => u16::from(*glyphs.get(usize::try_from(codepoint).ok()?)?),
            Kind::SegmentDelta(format4) => format4.map(codepoint)?,
            Kind::Trimmed { first, glyphs } => glyphs.get(codepoint.checked_sub(first)? as usize)?,
            Kind::Groups(groups) => groups.map(codepoint)?,
            Kind::Other => return None,
        };
        (glyph != 0).then_some(GlyphId(glyph))
    }

    pub fn for_each_mapping(&self, mut f: impl FnMut(u32, GlyphId)) {
        match self.kind {
            Kind::ByteEncoding(glyphs) => {
                for (codepoint, &glyph) in (0u32..).zip(glyphs) {
                    if glyph != 0 {
                        f(codepoint, GlyphId(u16::from(glyph)));
                    }
                }
            }
            Kind::SegmentDelta(format4) => format4.for_each_mapping(f),
            Kind::Trimmed { first, glyphs } => {
                for (codepoint, glyph) in (first..).zip(glyphs) {
                    if glyph != 0 {
                        f(codepoint, GlyphId(glyph));
                    }
                }
            }
            Kind::Groups(groups) => groups.for_each_mapping(f),
            Kind::Other => {}
        }
    }
}
