use std::io;

#[derive(Debug, thiserror::Error)]
pub(super) enum StartFailure {
    #[error("failed to capture desktop output: {0}")]
    Capture(io::Error),
    #[error("failed to spawn desktop: {0}")]
    Spawn(io::Error),
    #[error("failed to watch desktop process: {0}")]
    Watch(io::Error),
}
