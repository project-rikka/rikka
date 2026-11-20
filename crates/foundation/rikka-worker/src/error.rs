#[derive(Debug, thiserror::Error)]
pub enum Error<E> {
    #[error("failed to spawn worker thread: {0}")]
    Spawn(#[source] std::io::Error),

    #[error(transparent)]
    Startup(E),

    #[error("worker thread exited during startup")]
    Exited,
}

pub type Result<T, E> = std::result::Result<T, Error<E>>;

#[derive(Debug, thiserror::Error)]
pub enum SendError {
    #[error("worker stopped")]
    Disconnected,

    #[error("worker reply timed out")]
    Timeout,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
