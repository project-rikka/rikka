#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GlyphKind {
    Mask,
    Color,
}

impl GlyphKind {
    #[must_use]
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Mask => 1,
            Self::Color => 4,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphImage {
    pub kind: GlyphKind,
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl GlyphImage {
    pub(crate) fn from_raster(image: &rikka_text_raster::GlyphImage) -> Option<Self> {
        if image.is_empty() {
            return None;
        }
        let kind = match image.kind() {
            rikka_text_raster::GlyphImageKind::Mask => GlyphKind::Mask,
            rikka_text_raster::GlyphImageKind::Color => GlyphKind::Color,
        };
        Some(Self {
            kind,
            left: image.left(),
            top: image.top(),
            width: image.width(),
            height: image.height(),
            pixels: image.pixels().to_vec(),
        })
    }
}
