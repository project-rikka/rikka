use crate::key::Key;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("not a Photoshop document")]
    NotPhotoshop,
    #[error("unsupported file version {0}")]
    UnsupportedVersion(u16),
    #[error("unexpected end of data: {needed} bytes needed at offset {offset}")]
    Truncated { offset: u64, needed: u64 },
    #[error("malformed {what} at offset {offset}")]
    Malformed { what: &'static str, offset: u64 },
    #[error("invalid {what}: {value}")]
    Invalid { what: &'static str, value: i64 },
    #[error("expected signature `{expected}`, found `{found}`")]
    Signature { expected: Key, found: Key },
    #[error("unknown descriptor type `{0}`")]
    DescriptorType(Key),
    #[error("{width}x{height} exceeds the format's {limit} pixel limit")]
    TooLarge { width: u32, height: u32, limit: u32 },
    #[error("decoded image needs {needed} bytes, limit is {limit}")]
    MemoryLimit { needed: u64, limit: u64 },
    #[error("compressed channel data is corrupt")]
    CorruptChannel,
    #[error("section too large for its length field")]
    Oversized,
    #[error("unsupported colour mode {0}")]
    ColorMode(&'static str),
    #[error("channel planes do not match the document: {0}")]
    Planes(&'static str),
    #[error("text cannot be encoded: {0}")]
    Text(&'static str),
    #[error("colour management failed: {0}")]
    ColorManagement(String),
    #[error("the layer tree has no place at that path")]
    NoPlace,
    #[error("{0}")]
    Io(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Io(error.to_string())
    }
}
