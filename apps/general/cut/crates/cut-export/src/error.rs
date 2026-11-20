use std::convert::Infallible;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Render(#[from] cut_render::Error),

    #[error(transparent)]
    Imaging(#[from] rikka_image_edit::Error),

    #[error(transparent)]
    Video(#[from] rikka_video::Error),

    #[error(transparent)]
    Audio(#[from] rikka_audio_decode::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("cannot start the export: {0}")]
    Spawn(#[from] rikka_worker::Error<Infallible>),

    #[error("encoder stopped before the last frame")]
    Encoder,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
