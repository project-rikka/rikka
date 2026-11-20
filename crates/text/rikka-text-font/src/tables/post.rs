use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::sfnt::table_id::TableId;
use crate::tables::mac_glyph_names::MAC_GLYPH_NAMES;

#[derive(Clone, Copy, Debug)]
pub struct Post<'a> {
    data: &'a [u8],
}

impl<'a> Post<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        data.range(0, 32)?;
        Some(Self { data })
    }

    pub fn version(&self) -> u32 {
        self.data.u32_at(0).unwrap_or(0)
    }

    pub fn italic_angle(&self) -> f32 {
        self.data.fixed_at(4).unwrap_or(0.0)
    }

    pub fn underline_position(&self) -> i16 {
        self.data.i16_at(8).unwrap_or(0)
    }

    pub fn underline_thickness(&self) -> i16 {
        self.data.i16_at(10).unwrap_or(0)
    }

    pub fn is_fixed_pitch(&self) -> bool {
        self.data.u32_at(12).is_some_and(|v| v != 0)
    }

    pub fn glyph_name(&self, glyph: GlyphId) -> Option<&'a str> {
        match self.version() {
            0x0001_0000 => MAC_GLYPH_NAMES.get(glyph.index()).copied(),
            0x0002_0000 => self.version2_name(glyph),
            _ => None,
        }
    }

    fn version2_name(&self, glyph: GlyphId) -> Option<&'a str> {
        let count = self.data.u16_at(32)?;
        if glyph.0 >= count {
            return None;
        }
        let index = usize::from(self.data.u16_at(34 + glyph.index() * 2)?);
        if let Some(name) = MAC_GLYPH_NAMES.get(index) {
            return Some(name);
        }
        let mut offset = 34 + usize::from(count) * 2;
        for _ in 0..index - MAC_GLYPH_NAMES.len() {
            offset += 1 + usize::from(self.data.u8_at(offset)?);
        }
        let len = usize::from(self.data.u8_at(offset)?);
        std::str::from_utf8(self.data.range(offset + 1, len)?).ok()
    }
}

impl<'a> Font<'a> {
    pub fn post(&self) -> Option<Post<'a>> {
        Post::new(self.table_data(TableId::Post)?)
    }
}
