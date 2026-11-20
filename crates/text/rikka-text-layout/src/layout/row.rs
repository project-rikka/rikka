use std::ops::Range;

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub(crate) top: f32,
    pub(crate) height: f32,
    pub(crate) baseline: f32,
    pub(crate) left: f32,
    pub(crate) width: f32,
    pub(crate) origin: f32,
    pub(crate) ascent: f32,
    pub(crate) descent: f32,
    pub(crate) text: Range<u32>,
    pub(crate) glyphs: Range<u32>,
    pub(crate) decorations: Range<u32>,
    pub(crate) rtl: bool,
    pub(crate) ellipsis: Option<&'static str>,
}

impl Row {
    #[inline]
    pub fn top(&self) -> f32 {
        self.top
    }

    #[inline]
    pub fn height(&self) -> f32 {
        self.height
    }

    #[inline]
    pub fn bottom(&self) -> f32 {
        self.top + self.height
    }

    #[inline]
    pub fn baseline(&self) -> f32 {
        self.baseline
    }

    #[inline]
    pub fn left(&self) -> f32 {
        self.left
    }

    #[inline]
    pub fn width(&self) -> f32 {
        self.width
    }

    #[inline]
    pub fn right(&self) -> f32 {
        self.left + self.width
    }

    #[inline]
    pub fn ascent(&self) -> f32 {
        self.ascent
    }

    #[inline]
    pub fn descent(&self) -> f32 {
        self.descent
    }

    #[inline]
    pub fn text_range(&self) -> Range<usize> {
        self.text.start as usize..self.text.end as usize
    }

    #[inline]
    pub fn is_rtl(&self) -> bool {
        self.rtl
    }

    #[inline]
    pub fn is_elided(&self) -> bool {
        self.ellipsis.is_some()
    }

    #[inline]
    pub fn ellipsis(&self) -> Option<&'static str> {
        self.ellipsis
    }

    pub(crate) fn glyph_range(&self) -> Range<usize> {
        self.glyphs.start as usize..self.glyphs.end as usize
    }

    pub(crate) fn decoration_range(&self) -> Range<usize> {
        self.decorations.start as usize..self.decorations.end as usize
    }
}
