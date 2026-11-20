use std::convert::Infallible;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot start the waveform decoders: {0}")]
    Spawn(rikka_worker::Error<Infallible>),
    #[error("cannot read {}: {error}", path.display())]
    Read {
        path: PathBuf,
        error: std::io::Error,
    },
    #[error("malformed waveform cache")]
    Malformed,
    #[error("unreadable waveform cache: {0}")]
    Format(#[from] rikka_file_format::DecodeError),
    #[error("waveform cache is stale")]
    Stale,
    #[error(transparent)]
    Sound(#[from] rikka_audio_decode::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
