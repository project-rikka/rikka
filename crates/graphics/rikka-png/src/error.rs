use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkKind(pub [u8; 4]);

impl ChunkKind {
    #[must_use]
    pub const fn is_critical(self) -> bool {
        self.0[0].is_ascii_uppercase()
    }
}

impl fmt::Display for ChunkKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{}", byte.escape_ascii())?;
        }
        Ok(())
    }
}

impl fmt::Debug for ChunkKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ChunkKind({self})")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("not a PNG stream")]
    Signature,
    #[error("PNG stream ends early")]
    Truncated,
    #[error("invalid PNG image header")]
    Header,
    #[error("malformed {0} chunk")]
    Chunk(ChunkKind),
    #[error("{0} chunk does not match its checksum")]
    Checksum(ChunkKind),
    #[error("unknown critical chunk {0}")]
    UnknownCriticalChunk(ChunkKind),
    #[error("PNG stream has no image data")]
    NoImageData,
    #[error("image data: {0}")]
    Data(#[from] rikka_deflate::Error),
    #[error("unknown row filter {0}")]
    Filter(u8),
    #[error("{width}x{height} image exceeds the decoding limits")]
    LimitsExceeded { width: u32, height: u32 },
    #[error("invalid image size {width}x{height}")]
    InvalidSize { width: u32, height: u32 },
    #[error("pixel data has {len} bytes, expected {expected}")]
    PixelDataSize { len: usize, expected: usize },
    #[error("encoded image exceeds the PNG chunk size")]
    TooLarge,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
