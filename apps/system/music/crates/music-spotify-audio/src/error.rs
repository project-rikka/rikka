use std::io;

use rikka_file_format::DecodeError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read kept Spotify stream: {0}")]
    Read(#[from] io::Error),

    #[error(transparent)]
    Label(#[from] DecodeError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
