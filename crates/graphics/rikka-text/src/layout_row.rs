use std::ops::Range;

use rikka_text_layout::{Decoration, Glyph, ParagraphLayout, Row};

use crate::glyph_key::GlyphKey;

#[derive(Clone, Copy, Debug)]
pub struct LayoutRow<'a> {
    pub(crate) index: usize,
    pub(crate) line: usize,
    pub(crate) line_start: usize,
    pub(crate) top: f32,
    pub(crate) layout: &'a ParagraphLayout,
    pub(crate) row: &'a Row,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedGlyph {
    pub key: GlyphKey,
    pub x: i32,
    pub y: i32,
    pub offset: Option<usize>,
}

impl<'a> LayoutRow<'a> {
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    pub fn line(&self) -> usize {
        self.line
    }

    #[must_use]
    pub fn line_start(&self) -> usize {
        self.line_start
    }

    #[must_use]
    pub fn top(&self) -> f32 {
        self.top
    }

    #[must_use]
    pub fn height(&self) -> f32 {
        self.row.height()
    }

    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.top + self.row.height()
    }

    #[must_use]
    pub fn baseline(&self) -> f32 {
        self.top + self.row.baseline() - self.row.top()
    }

    #[must_use]
    pub fn left(&self) -> f32 {
        self.row.left()
    }

    #[must_use]
    pub fn width(&self) -> f32 {
        self.row.width()
    }

    #[must_use]
    pub fn right(&self) -> f32 {
        self.row.right()
    }

    #[must_use]
    pub fn text_range(&self) -> Range<usize> {
        let range = self.row.text_range();
        range.start + self.line_start..range.end + self.line_start
    }

    #[must_use]
    pub fn is_rtl(&self) -> bool {
        self.row.is_rtl()
    }

    #[must_use]
    pub fn is_elided(&self) -> bool {
        self.row.is_elided()
    }

    #[must_use]
    pub fn glyphs(&self) -> &'a [Glyph] {
        self.layout.glyphs(self.row)
    }

    #[must_use]
    pub fn decorations(&self) -> &'a [Decoration] {
        self.layout.decorations(self.row)
    }

    #[must_use]
    pub fn offset_of(&self, glyph: &Glyph) -> Option<usize> {
        (glyph.start < glyph.end).then(|| self.line_start + glyph.start as usize)
    }

    pub fn placed(&self, origin: (f32, f32), scale: f32) -> impl Iterator<Item = PlacedGlyph> + 'a {
        let row = *self;
        self.glyphs().iter().map(move |glyph| {
            let (key, x, y) = GlyphKey::place(glyph, origin, scale);
            PlacedGlyph {
                key,
                x,
                y,
                offset: row.offset_of(glyph),
            }
        })
    }
}
