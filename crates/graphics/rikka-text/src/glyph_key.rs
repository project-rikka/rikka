use rikka_svg::ColorScheme;
use rikka_text_catalog::FaceId;
use rikka_text_font::{GlyphId, Synthesis};
use rikka_text_layout::{Glyph, LocationId};
use rikka_text_raster::GlyphRequest;

const SUBPIXEL_BINS: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    face: FaceId,
    location: LocationId,
    size_bits: u32,
    glyph: u16,
    x_bin: u8,
    color: bool,
    synthesis: Synthesis,
    scheme: ColorScheme,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CurveKey {
    face: FaceId,
    location: LocationId,
    glyph: u16,
    synthesis: Synthesis,
}

impl GlyphKey {
    #[must_use]
    pub fn place(glyph: &Glyph, origin: (f32, f32), scale: f32) -> (GlyphKey, i32, i32) {
        let x = origin.0 + glyph.x * scale;
        let quarters = (x * SUBPIXEL_BINS).round();
        let whole = (quarters / SUBPIXEL_BINS).floor();
        let x_bin = (quarters - whole * SUBPIXEL_BINS) as u8;
        let y = (origin.1 + glyph.y * scale).round();
        let key = GlyphKey {
            face: glyph.face,
            location: glyph.location,
            size_bits: (glyph.font_size * scale).to_bits(),
            glyph: glyph.id,
            x_bin,
            color: glyph.color,
            synthesis: glyph.synthesis,
            scheme: ColorScheme::default(),
        };
        (key, whole as i32, y as i32)
    }

    #[must_use]
    pub fn face(&self) -> FaceId {
        self.face
    }

    #[must_use]
    pub fn glyph(&self) -> u16 {
        self.glyph
    }

    #[must_use]
    pub fn location(&self) -> LocationId {
        self.location
    }

    #[must_use]
    pub fn size(&self) -> f32 {
        f32::from_bits(self.size_bits)
    }

    #[must_use]
    pub fn resized(self, size: f32) -> GlyphKey {
        GlyphKey {
            size_bits: size.to_bits(),
            ..self
        }
    }

    #[must_use]
    pub fn with_color_scheme(self, scheme: ColorScheme) -> GlyphKey {
        GlyphKey { scheme, ..self }
    }

    #[must_use]
    pub fn color_scheme(&self) -> ColorScheme {
        self.scheme
    }

    #[must_use]
    pub fn subpixel_x(&self) -> f32 {
        f32::from(self.x_bin) / SUBPIXEL_BINS
    }

    #[must_use]
    pub fn is_color(&self) -> bool {
        self.color
    }

    #[must_use]
    pub fn synthesis(&self) -> Synthesis {
        self.synthesis
    }

    #[must_use]
    pub fn request(&self) -> GlyphRequest<'static> {
        GlyphRequest::new(GlyphId(self.glyph), self.size())
            .location(self.location.location())
            .subpixel_x(self.subpixel_x())
            .synthesis(self.synthesis)
            .color(self.color)
    }

    #[must_use]
    pub fn curve_key(&self) -> CurveKey {
        CurveKey {
            face: self.face,
            location: self.location,
            glyph: self.glyph,
            synthesis: self.synthesis,
        }
    }
}

impl CurveKey {
    #[must_use]
    pub fn face(&self) -> FaceId {
        self.face
    }

    #[must_use]
    pub fn glyph(&self) -> u16 {
        self.glyph
    }

    #[must_use]
    pub fn location(&self) -> LocationId {
        self.location
    }

    #[must_use]
    pub fn synthesis(&self) -> Synthesis {
        self.synthesis
    }

    #[must_use]
    pub fn request(&self, ppem: f32) -> GlyphRequest<'static> {
        GlyphRequest::new(GlyphId(self.glyph), ppem)
            .location(self.location.location())
            .synthesis(self.synthesis)
    }
}
