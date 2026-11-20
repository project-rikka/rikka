#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Opus carries one or two channels, not {0}")]
    Channels(u16),

    #[error("{samples} samples are not whole frames of {channels} channels")]
    PartialFrame { samples: usize, channels: usize },

    #[error("libopus: {0}")]
    Library(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
