#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no such process: {0}")]
    NoSuchProcess(i32),
    #[error("permission denied to signal process {0}")]
    PermissionDenied(i32),
    #[error("cannot signal process {pid}: {source}")]
    Signal {
        pid: i32,
        #[source]
        source: std::io::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
