use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rikka_format::numbered_name;

use crate::error::{Error, Result};
use crate::info::{Info, info_name};
use crate::store::Store;

#[derive(Clone, Debug)]
pub struct TrashEntry {
    pub path: PathBuf,
    pub original: PathBuf,
    pub deleted: Option<SystemTime>,
}

#[must_use]
pub fn can_trash(path: &Path) -> bool {
    Store::for_path(path).is_ok()
}

pub fn trash(path: &Path) -> Result<PathBuf> {
    if std::fs::symlink_metadata(path).is_err() {
        return Err(Error::NotFound(path.to_owned()));
    }
    let store = Store::for_path(path)?;
    store.ensure()?;

    let original = path.file_name().unwrap_or(OsStr::new("file"));
    let (name, info_path) = reserve_name(&store, original)?;

    let record = Info::new(store.record_path(path));
    if let Err(error) = std::fs::write(&info_path, record.to_file()) {
        discard_reserved_info(&info_path);
        return Err(Error::Io {
            path: info_path,
            source: error,
        });
    }

    let destination = store.files().join(&name);
    match rikka_fs::rename_new(path, &destination) {
        Ok(()) => Ok(destination),
        Err(error) => {
            discard_reserved_info(&info_path);
            Err(Error::Io {
                path: path.to_owned(),
                source: error,
            })
        }
    }
}

fn reserve_name(store: &Store, original: &OsStr) -> Result<(OsString, PathBuf)> {
    let readable = original.to_string_lossy().into_owned();
    let candidates = std::iter::once(original.to_owned())
        .chain((2..).map(|index| OsString::from(numbered_name(&readable, index))));
    for candidate in candidates {
        if std::fs::symlink_metadata(store.files().join(&candidate)).is_ok() {
            continue;
        }
        let info_path = store.info().join(info_name(Path::new(&candidate)));
        match std::fs::File::create_new(&info_path) {
            Ok(_) => return Ok((candidate, info_path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(Error::Io {
                    path: info_path,
                    source: error,
                });
            }
        }
    }
    unreachable!("candidate names are unbounded")
}

#[must_use]
pub fn entries() -> Vec<TrashEntry> {
    let mut found = Vec::new();
    for store in Store::all() {
        let Ok(reading) = std::fs::read_dir(store.files()) else {
            continue;
        };
        for entry in reading.flatten() {
            let name = entry.file_name();
            let info_path = store.info().join(info_name(Path::new(&name)));
            let Ok(text) = std::fs::read_to_string(&info_path) else {
                continue;
            };
            let Some(record) = Info::parse(&text) else {
                continue;
            };
            found.push(TrashEntry {
                path: entry.path(),
                original: store.resolve_path(record.path()),
                deleted: record.deleted_at(),
            });
        }
    }
    found
}

#[must_use]
pub fn holds(path: &Path) -> bool {
    Store::containing(path).is_some_and(|store| path != store.files())
}

#[must_use]
pub fn is_top_level(path: &Path) -> bool {
    Store::containing(path).is_some_and(|store| path.parent() == Some(store.files().as_path()))
}

pub fn restore(path: &Path) -> Result<PathBuf> {
    let store = Store::containing(path).ok_or_else(|| Error::Unsupported(path.to_owned()))?;
    let name = path
        .file_name()
        .ok_or_else(|| Error::NotFound(path.to_owned()))?;
    let info_path = store.info().join(info_name(Path::new(name)));
    let text = std::fs::read_to_string(&info_path).map_err(Error::io(&info_path))?;
    let record = Info::parse(&text).ok_or_else(|| Error::NotFound(info_path.clone()))?;

    let original = store.resolve_path(record.path());
    if let Some(parent) = original.parent() {
        std::fs::create_dir_all(parent).map_err(Error::io(parent))?;
    }
    rikka_fs::rename_new(path, &original).map_err(|error| match error.kind() {
        std::io::ErrorKind::AlreadyExists => Error::Exists(original.clone()),
        _ => Error::Io {
            path: path.to_owned(),
            source: error,
        },
    })?;
    remove_info(&info_path)?;
    Ok(original)
}

pub fn erase(path: &Path) -> Result<()> {
    let store = Store::containing(path)
        .filter(|store| path != store.files())
        .ok_or_else(|| Error::Unsupported(path.to_owned()))?;
    let metadata = std::fs::symlink_metadata(path).map_err(Error::io(path))?;
    let removed = match metadata.is_dir() && !metadata.is_symlink() {
        true => std::fs::remove_dir_all(path),
        false => std::fs::remove_file(path),
    };
    removed.map_err(Error::io(path))?;
    if path.parent() == Some(store.files().as_path())
        && let Some(name) = path.file_name()
    {
        remove_info(&store.info().join(info_name(Path::new(name))))?;
    }
    Ok(())
}

fn remove_info(info_path: &Path) -> Result<()> {
    match std::fs::remove_file(info_path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(Error::Io {
            path: info_path.to_owned(),
            source: error,
        }),
        _ => Ok(()),
    }
}

fn discard_reserved_info(info_path: &Path) {
    drop(std::fs::remove_file(info_path));
}
