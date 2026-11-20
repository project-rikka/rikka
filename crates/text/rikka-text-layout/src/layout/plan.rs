use std::ops::Range;

use rikka_text_unicode::{Level, Script};

use crate::shaping::{GlyphColumns, Shaped};

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RowPlan {
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) content_end: u32,
    pub(crate) separators: u32,
    pub(crate) mandatory: bool,
    pub(crate) pieces: Range<u32>,
    pub(crate) content_width: f32,
    pub(crate) hang_width: f32,
    pub(crate) ellipsis: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Shaped,
    Fragment,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Piece {
    pub(crate) source: Source,
    pub(crate) glyphs: Range<u32>,
    pub(crate) face: u16,
    pub(crate) style: u16,
    pub(crate) level: Level,
    pub(crate) script: Script,
    pub(crate) hanging: bool,
    pub(crate) width: f32,
}

impl Piece {
    #[inline]
    pub(crate) fn glyph_range(&self) -> Range<usize> {
        self.glyphs.start as usize..self.glyphs.end as usize
    }

    #[inline]
    pub(crate) fn columns<'a>(
        &self,
        shaped: &'a Shaped,
        fragments: &'a GlyphColumns,
    ) -> &'a GlyphColumns {
        match self.source {
            Source::Shaped => &shaped.glyphs,
            Source::Fragment => fragments,
        }
    }
}
