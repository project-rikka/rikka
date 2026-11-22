use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Rikka does not manage this system's configuration: {} not found", .0.display())]
    Unmanaged(PathBuf),
    #[error("{}: {source}", path.display())]
    Malformed {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("not an absolute path without `.` or `..`: {}", .0.display())]
    NotAbsolute(PathBuf),
    #[error("{} is not inside the flake {}", modules.display(), flake.display())]
    Outside { modules: PathBuf, flake: PathBuf },
    #[error("no flake.nix in {}", .0.display())]
    NoFlake(PathBuf),
    #[error("{} does not hold Rikka's modules", .0.display())]
    NotAdopted(PathBuf),
    #[error("failed to read {}: {source}", path.display())]
    Unreadable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{}: {source}", path.display())]
    Invalid {
        path: PathBuf,
        #[source]
        source: rikka_nix_edit::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
