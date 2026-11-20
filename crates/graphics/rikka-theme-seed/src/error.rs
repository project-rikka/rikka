use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read `{path}`")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to decode `{path}` as a picture or video")]
    Decode {
        path: PathBuf,
        #[source]
        source: rikka_media::Error,
    },

    #[error("`{path}` has no pixels")]
    Empty { path: PathBuf },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
