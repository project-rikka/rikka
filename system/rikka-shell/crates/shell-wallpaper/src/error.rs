use std::path::PathBuf;

use rikka_seam::SeamError;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("wallpaper folder unavailable: {0}")]
    Unavailable(&'static str),

    #[error("no XDG pictures directory for wallpapers")]
    NoPictures,

    #[error("wallpaper folder unavailable")]
    Folder(#[source] rikka_dirs::Error),

    #[error("failed to read `{path}`")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to make thumbnail of `{path}`")]
    Thumbnail {
        path: PathBuf,
        #[source]
        source: rikka_image::Error,
    },

    #[error("failed to write `{path}`")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

impl From<Error> for SeamError {
    fn from(error: Error) -> SeamError {
        SeamError::Failed(error.to_string())
    }
}
