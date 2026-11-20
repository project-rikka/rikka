#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("PipeWire is unreachable: {0}")]
    Unreachable(String),
    #[error("{path}: {reason}")]
    Unwritable { path: String, reason: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn startup(error: rikka_worker::Error<Error>) -> Error {
    match error {
        rikka_worker::Error::Startup(error) => error,
        other => Error::Unreachable(other.to_string()),
    }
}
