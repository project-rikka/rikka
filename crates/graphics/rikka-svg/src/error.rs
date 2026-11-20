#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("malformed SVG document: {0}")]
    Malformed(String),

    #[error("SVG document exceeds the element limit")]
    TooComplex,

    #[error("SVG document has no valid size")]
    NoSize,

    #[error("invalid raster size {width}x{height}")]
    InvalidSize { width: u32, height: u32 },

    #[error("{width}x{height} raster has {pixels} pixels, limit is {limit}")]
    TooManyPixels {
        width: u32,
        height: u32,
        pixels: u64,
        limit: u64,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
