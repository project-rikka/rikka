use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::table_id::TableId;
use crate::sfnt::tag::Tag;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Selection(pub u16);

impl Selection {
    pub const ITALIC: u16 = 1 << 0;
    pub const UNDERSCORE: u16 = 1 << 1;
    pub const NEGATIVE: u16 = 1 << 2;
    pub const OUTLINED: u16 = 1 << 3;
    pub const STRIKEOUT: u16 = 1 << 4;
    pub const BOLD: u16 = 1 << 5;
    pub const REGULAR: u16 = 1 << 6;
    pub const USE_TYPO_METRICS: u16 = 1 << 7;
    pub const WWS: u16 = 1 << 8;
    pub const OBLIQUE: u16 = 1 << 9;

    #[inline]
    pub fn contains(self, bit: u16) -> bool {
        self.0 & bit != 0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScriptPlacement {
    pub x_size: i16,
    pub y_size: i16,
    pub x_offset: i16,
    pub y_offset: i16,
}

#[derive(Clone, Copy, Debug)]
pub struct Os2<'a> {
    data: &'a [u8],
}

impl<'a> Os2<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        data.range(0, 68)?;
        Some(Self { data })
    }

    #[inline]
    fn i16(&self, offset: usize) -> i16 {
        self.data.i16_at(offset).unwrap_or(0)
    }

    #[inline]
    fn u16(&self, offset: usize) -> u16 {
        self.data.u16_at(offset).unwrap_or(0)
    }

    pub fn version(&self) -> u16 {
        self.u16(0)
    }

    pub fn average_char_width(&self) -> i16 {
        self.i16(2)
    }

    pub fn weight_class(&self) -> u16 {
        self.u16(4)
    }

    pub fn width_class(&self) -> u16 {
        self.u16(6)
    }

    pub fn embedding_flags(&self) -> u16 {
        self.u16(8)
    }

    pub fn subscript(&self) -> ScriptPlacement {
        self.placement(10)
    }

    pub fn superscript(&self) -> ScriptPlacement {
        self.placement(18)
    }

    fn placement(&self, offset: usize) -> ScriptPlacement {
        ScriptPlacement {
            x_size: self.i16(offset),
            y_size: self.i16(offset + 2),
            x_offset: self.i16(offset + 4),
            y_offset: self.i16(offset + 6),
        }
    }

    pub fn strikeout_size(&self) -> i16 {
        self.i16(26)
    }

    pub fn strikeout_position(&self) -> i16 {
        self.i16(28)
    }

    pub fn family_class(&self) -> i16 {
        self.i16(30)
    }

    pub fn panose(&self) -> [u8; 10] {
        self.data.chunk(32).unwrap_or_default()
    }

    pub fn unicode_ranges(&self) -> [u32; 4] {
        [42, 46, 50, 54].map(|offset| self.data.u32_at(offset).unwrap_or(0))
    }

    pub fn vendor(&self) -> Tag {
        self.data.tag_at(58).unwrap_or_default()
    }

    pub fn selection(&self) -> Selection {
        Selection(self.u16(62))
    }

    pub fn first_char(&self) -> u16 {
        self.u16(64)
    }

    pub fn last_char(&self) -> u16 {
        self.u16(66)
    }

    pub fn typo_ascender(&self) -> Option<i16> {
        self.data.i16_at(68)
    }

    pub fn typo_descender(&self) -> Option<i16> {
        self.data.i16_at(70)
    }

    pub fn typo_line_gap(&self) -> Option<i16> {
        self.data.i16_at(72)
    }

    pub fn win_ascent(&self) -> Option<u16> {
        self.data.u16_at(74)
    }

    pub fn win_descent(&self) -> Option<u16> {
        self.data.u16_at(76)
    }

    pub fn code_page_ranges(&self) -> Option<[u32; 2]> {
        if self.version() < 1 {
            return None;
        }
        Some([self.data.u32_at(78)?, self.data.u32_at(82)?])
    }

    pub fn x_height(&self) -> Option<i16> {
        self.v2_field(86)
    }

    pub fn cap_height(&self) -> Option<i16> {
        self.v2_field(88)
    }

    pub fn default_char(&self) -> Option<u16> {
        self.v2_field(90).map(|v| v as u16)
    }

    pub fn break_char(&self) -> Option<u16> {
        self.v2_field(92).map(|v| v as u16)
    }

    pub fn max_context(&self) -> Option<u16> {
        self.v2_field(94).map(|v| v as u16)
    }

    fn v2_field(&self, offset: usize) -> Option<i16> {
        if self.version() < 2 {
            return None;
        }
        self.data.i16_at(offset)
    }

    pub fn optical_point_size_range(&self) -> Option<(f32, f32)> {
        if self.version() < 5 {
            return None;
        }
        let lower = self.data.u16_at(96)?;
        let upper = self.data.u16_at(98)?;
        Some((f32::from(lower) / 20.0, f32::from(upper) / 20.0))
    }
}

impl<'a> Font<'a> {
    pub fn os2(&self) -> Option<Os2<'a>> {
        Os2::new(self.table_data(TableId::Os2)?)
    }
}
