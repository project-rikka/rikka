use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

pub const SOCKET_ENV: &str = "RIKKA_SEAM_SOCKET";

const DISPLAY_ENV: &str = "WAYLAND_DISPLAY";
const SOCKET_EXTENSION: &str = "sock";
const SOCKET_NAME: &str = "seam.sock";

#[must_use]
pub fn socket_path() -> Option<PathBuf> {
    if let Some(named) = env::var_os(SOCKET_ENV) {
        return Some(PathBuf::from(named));
    }
    socket_path_for(env::var_os(DISPLAY_ENV).unwrap_or_else(|| "wayland-0".into()))
}

#[must_use]
pub fn socket_path_for(display: impl AsRef<OsStr>) -> Option<PathBuf> {
    Some(
        sessions_directory()?
            .join(display.as_ref())
            .join(SOCKET_NAME),
    )
}

#[must_use]
pub fn socket_path_beside(name: &str) -> Option<PathBuf> {
    let session = socket_path()?;
    Some(session.parent()?.join(format!("{name}.{SOCKET_EXTENSION}")))
}

#[must_use]
pub fn session_sockets() -> Vec<PathBuf> {
    let Some(first) = socket_path() else {
        return Vec::new();
    };

    let mut beside: Vec<PathBuf> = first
        .parent()
        .and_then(|directory| fs::read_dir(directory).ok())
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path != &first && path.extension() == Some(OsStr::new(SOCKET_EXTENSION)))
        .collect();
    beside.sort();

    let mut sockets = vec![first];
    sockets.append(&mut beside);
    sockets
}

#[must_use]
pub fn sessions_directory() -> Option<PathBuf> {
    let paths = rikka_dirs::Paths::resolve().ok()?;
    Some(paths.runtime_dir()?.to_path_buf())
}

#[must_use]
pub fn display_of(socket: &Path) -> Option<&OsStr> {
    socket.parent()?.file_name()
}
