use std::path::PathBuf;

use crate::reach::Reach;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{} does not exist", .0.display())]
    NotFound(PathBuf),

    #[error("inotify watch limit reached at {}", .0.display())]
    Limit(PathBuf),

    #[error("watching {reach:?} of {} is not supported on this platform", path.display())]
    Unsupported { path: PathBuf, reach: Reach },

    #[error("failed to watch {}: {source}", path.display())]
    Watch {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to open file watcher: {0}")]
    Open(#[source] std::io::Error),

    #[error("failed to read file changes: {0}")]
    Read(#[source] std::io::Error),

    #[error("failed to start watcher thread: {0}")]
    Spawn(#[source] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
