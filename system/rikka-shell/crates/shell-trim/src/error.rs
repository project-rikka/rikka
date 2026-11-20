#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("cannot open recording: {0}")]
    Unplayable(String),

    #[error("recording duration unknown")]
    Unmeasured,

    #[error("failed to replace recording with trimmed file: {0}")]
    NotReplaced(#[source] std::io::Error),

    #[error(transparent)]
    Cutting(#[from] rikka_video::Error),

    #[error(transparent)]
    Replay(#[from] crate::replays::Error),
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
