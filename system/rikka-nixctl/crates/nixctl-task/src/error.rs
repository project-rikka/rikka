use std::io;
use std::process::ExitStatus;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to start {program}: {source}")]
    Spawn {
        program: String,
        #[source]
        source: io::Error,
    },
    #[error("failed to wait for {program}: {source}")]
    Wait {
        program: String,
        #[source]
        source: io::Error,
    },
    #[error("{program} failed: {status}")]
    ExitStatus { program: String, status: ExitStatus },
    #[error("failed to read {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("failed to write {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("{error}; failed to restore the previous files: {source}")]
    Restore {
        error: Box<Error>,
        #[source]
        source: io::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
