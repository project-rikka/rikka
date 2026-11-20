use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to compile keymap `{names}`")]
    UnknownLayout { names: String },
    #[error("failed to compile keymap text")]
    MalformedKeymap,
    #[error("keymap of {size} bytes exceeds the {limit} byte limit")]
    KeymapTooLarge { size: u64, limit: u64 },
    #[error("failed to read keymap")]
    UnreadableKeymap(#[source] io::Error),
    #[error("failed to create keymap file")]
    UnsharableKeymap(#[source] io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
