use std::ops::Range;

use rikka_text_unicode::{Level, Script};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Run {
    pub(crate) text: Range<u32>,
    pub(crate) glyphs: Range<u32>,
    pub(crate) face: u16,
    pub(crate) style: u16,
    pub(crate) level: Level,
    pub(crate) script: Script,
}

impl Run {
    #[inline]
    pub(crate) fn glyph_range(&self) -> Range<usize> {
        self.glyphs.start as usize..self.glyphs.end as usize
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RunStyle {
    pub(crate) size: Option<f32>,
    pub(crate) underline: bool,
    pub(crate) strikethrough: bool,
    pub(crate) primary: Option<u16>,
}

impl RunStyle {
    #[inline]
    pub(crate) fn size(&self, font_size: f32) -> f32 {
        self.size.unwrap_or(font_size)
    }
}
