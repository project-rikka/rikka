use std::fs::DirBuilder;
use std::io::ErrorKind;
use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::Command;

use rikka_dirs::{cache_home, runtime_dir};
use rikka_open::spawn_detached;

use crate::error::{Error, Result};

const VIEWER: &str = "rikka-viewer";
const HOME: &str = "rikka-proton/viewing";
const DIRECTORY_MODE: u32 = 0o700;

pub(crate) fn private_dir() -> Result<PathBuf> {
    let home = home().ok_or(Error::NoRoom)?;
    let failed = |path: &Path| {
        let path = path.display().to_string();
        move |source| Error::File { path, source }
    };

    DirBuilder::new()
        .recursive(true)
        .mode(DIRECTORY_MODE)
        .create(&home)
        .map_err(failed(&home))?;
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(DIRECTORY_MODE))
        .map_err(failed(&home))?;

    (1u64..)
        .find_map(|count| {
            let room = home.join(count.to_string());
            match DirBuilder::new().mode(DIRECTORY_MODE).create(&room) {
                Ok(()) => Some(Ok(room)),
                Err(taken) if taken.kind() == ErrorKind::AlreadyExists => None,
                Err(source) => Some(Err(failed(&room)(source))),
            }
        })
        .unwrap_or(Err(Error::NoRoom))
}

pub fn clear_viewing_dir() {
    if let Some(home) = home() {
        drop(std::fs::remove_dir_all(home));
    }
}

pub fn show(path: &Path) -> Result<()> {
    spawn_detached(Command::new(VIEWER).arg(path))
        .map(drop)
        .map_err(Error::Viewer)
}

fn home() -> Option<PathBuf> {
    Some(runtime_dir().or_else(cache_home)?.join(HOME))
}
