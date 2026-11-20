use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to watch file system: {0}")]
    Watch(String),

    #[error("failed to spawn watcher thread: {0}")]
    Spawn(#[source] std::io::Error),

    #[error("failed to access recently used list {}", list.display())]
    Recent {
        list: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("malformed recently used list {}: {reason}", list.display())]
    RecentMalformed { list: PathBuf, reason: String },

    #[error("failed to access MIME associations {}", list.display())]
    Associations {
        list: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("MIME associations {} are read-only", list.display())]
    AssociationsReadOnly { list: PathBuf },

    #[error("invalid desktop file id or MIME type: {0:?}")]
    InvalidAssociation(String),

    #[error("failed to access preferred terminals {}", list.display())]
    Terminals {
        list: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("preferred terminals {} are read-only", list.display())]
    TerminalsReadOnly { list: PathBuf },

    #[error("invalid terminal desktop file id: {0:?}")]
    InvalidTerminal(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
