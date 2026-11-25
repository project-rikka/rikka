#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("NBT data ends early")]
    Truncated,
    #[error("unknown NBT tag kind {0}")]
    Kind(u8),
    #[error("the NBT root is not a compound")]
    Root,
    #[error("NBT nesting is deeper than {0}")]
    Depth(usize),
    #[error("invalid modified UTF-8 string")]
    Text,
    #[error("negative NBT length {0}")]
    Length(i32),
    #[error("NBT list of {expected:?} holds a {found:?}")]
    Mixed {
        expected: crate::Kind,
        found: crate::Kind,
    },
    #[error("NBT string of {0} bytes is too long")]
    LongText(usize),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
