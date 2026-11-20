#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("shell program not found")]
    ShellNotFound(#[source] std::io::Error),
    #[error("failed to open pty: {0}")]
    Pty(#[source] std::io::Error),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Error {
        match error.kind() {
            std::io::ErrorKind::NotFound => Error::ShellNotFound(error),
            _ => Error::Pty(error),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
