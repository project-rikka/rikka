#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EllipsisGlyph {
    pub(crate) id: u16,
    pub(crate) advance: f32,
    pub(crate) x_offset: f32,
    pub(crate) y_offset: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EllipsisGlyphs {
    pub(crate) text: &'static str,
    pub(crate) glyphs: Box<[EllipsisGlyph]>,
    pub(crate) advance: f32,
}
