#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("compressed stream is malformed")]
    Malformed,
    #[error("compressed stream ends early")]
    Truncated,
    #[error("decompressed data exceeds {limit} bytes")]
    TooLarge { limit: usize },
    #[error("decompressed data does not match its checksum")]
    Checksum,
    #[error("compressed stream has an unsupported header")]
    UnsupportedHeader,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
