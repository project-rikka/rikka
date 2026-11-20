use rikka_text_font::{Color, GlyphId, Location, Synthesis};

const OPAQUE_BLACK: Color = Color {
    r: 0,
    g: 0,
    b: 0,
    a: 255,
};

#[derive(Clone, Copy, Debug)]
pub struct GlyphRequest<'l> {
    pub(crate) glyph: GlyphId,
    pub(crate) ppem: f32,
    pub(crate) location: Option<&'l Location>,
    pub(crate) subpixel_x: f32,
    pub(crate) subpixel_y: f32,
    pub(crate) synthesis: Synthesis,
    pub(crate) foreground: Color,
    pub(crate) palette: u16,
    pub(crate) color: bool,
}

impl<'l> GlyphRequest<'l> {
    pub fn new(glyph: GlyphId, ppem: f32) -> Self {
        Self {
            glyph,
            ppem,
            location: None,
            subpixel_x: 0.0,
            subpixel_y: 0.0,
            synthesis: Synthesis::NONE,
            foreground: OPAQUE_BLACK,
            palette: 0,
            color: true,
        }
    }

    #[must_use]
    pub fn location(mut self, location: &'l Location) -> Self {
        self.location = Some(location);
        self
    }

    #[must_use]
    pub fn subpixel_x(mut self, offset: f32) -> Self {
        self.subpixel_x = fraction(offset);
        self
    }

    #[must_use]
    pub fn subpixel_y(mut self, rise: f32) -> Self {
        self.subpixel_y = fraction(rise);
        self
    }

    #[must_use]
    pub fn synthesis(mut self, synthesis: Synthesis) -> Self {
        self.synthesis = synthesis;
        self
    }

    #[must_use]
    pub fn foreground(mut self, color: Color) -> Self {
        self.foreground = color;
        self
    }

    #[must_use]
    pub fn palette(mut self, index: u16) -> Self {
        self.palette = index;
        self
    }

    #[must_use]
    pub fn color(mut self, enabled: bool) -> Self {
        self.color = enabled;
        self
    }

    #[inline]
    pub fn glyph(&self) -> GlyphId {
        self.glyph
    }

    #[inline]
    pub fn ppem(&self) -> f32 {
        self.ppem
    }

    #[inline]
    pub(crate) fn is_valid(&self) -> bool {
        self.ppem.is_finite() && self.ppem > 0.0
    }
}

fn fraction(offset: f32) -> f32 {
    if offset.is_finite() {
        offset - offset.floor()
    } else {
        0.0
    }
}
