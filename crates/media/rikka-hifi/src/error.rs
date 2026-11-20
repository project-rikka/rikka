#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to start hifi engine: {0}")]
    Start(String),

    #[error("failed to list sound cards: {0}")]
    ListDevices(String),

    #[error("source closed")]
    Closed,

    #[error("failed to read audio file: {0}")]
    Read(#[source] rikka_audio_decode::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
