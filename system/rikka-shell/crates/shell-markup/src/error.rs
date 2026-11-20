#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("no output directory: {0}")]
    NoFolder(#[source] std::io::Error),

    #[error(transparent)]
    Picture(#[from] rikka_image::Error),

    #[error(transparent)]
    Drawing(#[from] rikka_image_edit::Error),

    #[error("GPU lacks compute support for markup")]
    NoGpu,
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
