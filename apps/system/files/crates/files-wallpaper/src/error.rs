use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("there is no pictures folder")]
    NoPictures,

    #[error("failed to prepare wallpaper folder: {0}")]
    Folder(#[source] rikka_dirs::Error),

    #[error("failed to read `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("settings are unavailable: {0}")]
    Session(#[source] std::io::Error),

    #[error("session refused the wallpaper: {0}")]
    Refused(#[source] rikka_seam::SeamError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
