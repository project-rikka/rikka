use crate::sfnt::tag::Tag;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("not an OpenType font or font collection")]
    UnknownFormat,
    #[error("truncated or malformed font data")]
    Malformed,
    #[error("face index {index} out of range for collection of {count} faces")]
    FaceIndex { index: u32, count: u32 },
    #[error("glyph {glyph} out of range for font with {count} glyphs")]
    GlyphIndex { glyph: u16, count: u16 },
    #[error("missing `{0}` table")]
    MissingTable(Tag),
    #[error("cycle in COLR paint graph")]
    PaintCycle,
    #[error("COLR paint graph too deep or too large")]
    PaintTooComplex,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
