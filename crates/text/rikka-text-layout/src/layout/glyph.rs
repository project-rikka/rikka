use std::ops::Range;

use rikka_text_catalog::FaceId;
use rikka_text_unicode::Level;

use rikka_text_font::Synthesis;

use crate::fonts::LocationId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub face: FaceId,
    pub location: LocationId,
    pub id: u16,
    pub synthesis: Synthesis,
    pub color: bool,
    pub level: Level,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
    pub font_size: f32,
    pub start: u32,
    pub end: u32,
}

impl Glyph {
    #[inline]
    pub fn cluster(&self) -> Range<usize> {
        self.start as usize..self.end as usize
    }

    #[inline]
    pub fn is_rtl(&self) -> bool {
        self.level.is_rtl()
    }
}
