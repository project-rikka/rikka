use std::io;

use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot start {program}: {source}")]
    Spawn {
        program: String,
        #[source]
        source: io::Error,
    },

    #[error("`{command}` failed (exit {status})\n{stderr}")]
    Failed {
        command: String,
        status: i32,
        stderr: String,
    },

    #[error("{path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("cannot parse output of {0}")]
    Parse(String),

    #[error("{0}")]
    Answers(#[from] installer_core::Error),
}
