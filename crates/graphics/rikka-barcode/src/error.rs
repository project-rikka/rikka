#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("picture has no pixels")]
    Empty,

    #[error("picture too large: {width}×{height}")]
    TooLarge { width: u32, height: u32 },

    #[error("{width}×{height} RGBA picture needs {} bytes, got {length}", u64::from(*width) * u64::from(*height) * 4)]
    Mismatched {
        width: u32,
        height: u32,
        length: usize,
    },

    #[error("barcode reader failed: {0}")]
    Reader(String),

    #[error("{0} bytes of text do not fit in a barcode")]
    TooLong(usize),

    #[error("barcode writer failed: {0}")]
    Writer(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
