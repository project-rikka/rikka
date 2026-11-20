use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Picture(#[from] rikka_image::Error),

    #[error(transparent)]
    Sound(#[from] rikka_audio_decode::Error),

    #[error(transparent)]
    Movie(#[from] rikka_video::Error),

    #[error("\"{}\" is not an image, audio or video file", .0.display())]
    Unsupported(PathBuf),

    #[error("no still image in \"{}\"", .0.display())]
    NoPicture(PathBuf),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
