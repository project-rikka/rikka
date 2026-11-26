#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Meta(#[from] launcher_meta::Error),
    #[error("failed to start Java: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("failed to stop the game: {0}")]
    Stop(#[source] std::io::Error),
    #[error("an argument has an unmatched quote")]
    Quote,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
