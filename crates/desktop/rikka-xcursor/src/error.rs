use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read cursor file: {0}")]
    Read(#[from] io::Error),
    #[error("not an Xcursor file")]
    NotXcursor,
    #[error("Xcursor file ends inside {0}")]
    Truncated(&'static str),
    #[error("Xcursor file holds no image")]
    NoImage,
    #[error("Xcursor image of {width}x{height} exceeds the format's limit")]
    Oversized { width: u32, height: u32 },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
