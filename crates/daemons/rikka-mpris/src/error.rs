#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("session bus is unreachable: {0}")]
    Unreachable(String),
    #[error("failed to publish player on session bus: {0}")]
    Refused(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
