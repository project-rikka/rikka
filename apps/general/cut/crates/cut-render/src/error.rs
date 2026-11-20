use std::convert::Infallible;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to render frame: {0}")]
    Imaging(#[from] rikka_image_edit::Error),

    #[error("cannot start the preview renderer: {0}")]
    Spawn(#[from] rikka_worker::Error<Infallible>),

    #[error("timed out waiting for footage of frame {frame}")]
    Stalled { frame: i64 },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
