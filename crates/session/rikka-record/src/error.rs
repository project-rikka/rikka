#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Capture(#[from] rikka_capture::Error),

    #[error("encode: {0}")]
    Encode(#[from] rikka_video::Error),

    #[error("audio: {0}")]
    Audio(#[from] rikka_audio::Error),

    #[error("failed to spawn the recording thread: {0}")]
    Thread(#[source] std::io::Error),

    #[error("the recording thread stopped unexpectedly")]
    ThreadDied,

    #[error("the source is too small to encode")]
    TooSmall,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
