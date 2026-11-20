use std::io;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("document of {width}×{height} pixels exceeds the memory budget")]
    TooLarge { width: u32, height: u32 },
    #[error("cannot read the document")]
    Io(#[from] io::Error),
    #[error("cannot decode the document")]
    Psd(#[from] rikka_psd::Error),
}
