#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("invalid glyph request")]
    InvalidRequest,
    #[error("outline is not loaded")]
    NotLoaded,
    #[error("cannot load glyph outline: {0}")]
    Font(#[from] rikka_text_font::Error),
    #[error("glyph outline exceeds the point limit")]
    Truncated,
    #[error("glyph outline exceeds the rasterizer limits")]
    TooComplex,
    #[error("glyph SVG document is malformed")]
    MalformedSvg,
    #[error("font has no outline or bitmap for the glyph")]
    NoImage,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
