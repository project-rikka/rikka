use std::fs::{File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use rikka_dirs::UserDirs;
use rikka_proton_names::numbered_name;

use crate::error::{Error, Result};

const FALLBACK: &str = "file";
const MAX_NAME_NUMBER: u32 = 10_000;

#[must_use]
pub fn downloads() -> PathBuf {
    UserDirs::load().downloads()
}

pub fn save_download(name: &str, bytes: &[u8]) -> Result<PathBuf> {
    let into = downloads();
    std::fs::create_dir_all(&into).map_err(failed(&into))?;

    let (mut file, path) = create_unique(&into, &named(name)).map_err(failed(&into))?;
    if let Err(source) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(std::fs::remove_file(&path));
        return Err(failed(&path)(source));
    }

    Ok(path)
}

fn create_unique(into: &Path, name: &str) -> io::Result<(File, PathBuf)> {
    let numbered_names = (1..MAX_NAME_NUMBER).map(|number| numbered_name(name, number));
    for candidate in std::iter::once(name.to_owned()).chain(numbered_names) {
        let path = into.join(candidate);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(taken) if taken.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::from(io::ErrorKind::AlreadyExists))
}

fn named(name: &str) -> String {
    Path::new(name)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| FALLBACK.to_owned())
}

fn failed(path: &Path) -> impl FnOnce(io::Error) -> Error {
    let path = path.display().to_string();
    move |source| Error::File { path, source }
}
