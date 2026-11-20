#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("nothing to open")]
    NothingToOpen,

    #[error("{program} is not installed")]
    NotInstalled { program: String },

    #[error("failed to start {program}: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to open {target}: {source}")]
    Open {
        target: String,
        #[source]
        source: std::io::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
