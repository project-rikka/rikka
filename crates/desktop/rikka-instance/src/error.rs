#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to reach the running instance: {0}")]
    Call(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
