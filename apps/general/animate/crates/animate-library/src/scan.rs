use std::collections::{HashMap, HashSet};
use std::fs::Metadata;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::entry::Stamp;
use crate::format::Format;
use crate::location::{Layout, Location};
use crate::shelf::Shelf;

const DEEPEST: usize = 8;
const MOST_FILES: usize = 100_000;
const PICTURES: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
const FOLDER_PICTURES: [&str; 2] = ["preview.png", "thumbnail.png"];

pub(crate) struct Found {
    pub(crate) path: PathBuf,
    pub(crate) shelf: Shelf,
    pub(crate) folder: Arc<str>,
    pub(crate) format: Format,
    pub(crate) stamp: Stamp,
    pub(crate) sidecar: Option<PathBuf>,
}

pub(crate) fn scan(location: &Location) -> io::Result<Vec<Found>> {
    let mut walk = Walk::default();
    match location.layout {
        Layout::Shelf(shelf) => walk.folder(&location.root, shelf, "", 0)?,
        Layout::Library => {
            for (path, metadata) in listing(&location.root)? {
                let shelf = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .and_then(Shelf::from_folder);
                if let (true, Some(shelf)) = (metadata.is_dir(), shelf) {
                    walk.folder(&path, shelf, "", 0)?;
                }
            }
        }
    }
    Ok(walk.found)
}

#[derive(Default)]
struct Walk {
    found: Vec<Found>,
    visited: HashSet<PathBuf>,
    folders: HashMap<String, Arc<str>>,
    files: usize,
}

impl Walk {
    fn folder(&mut self, dir: &Path, shelf: Shelf, relative: &str, depth: usize) -> io::Result<()> {
        let canonical = std::fs::canonicalize(dir)?;
        if !self.visited.insert(canonical) {
            return Ok(());
        }
        let children = listing(dir)?;
        let names: HashMap<String, &Path> = children
            .iter()
            .filter(|(_, metadata)| metadata.is_file())
            .filter_map(|(path, _)| {
                let name = path.file_name()?.to_str()?.to_lowercase();
                Some((name, path.as_path()))
            })
            .collect();
        let files: Vec<(&PathBuf, &Metadata, Format)> = children
            .iter()
            .filter(|(_, metadata)| metadata.is_file())
            .filter_map(|(path, metadata)| Some((path, metadata, Format::from_path(path)?)))
            .collect();
        let models = files
            .iter()
            .filter(|(_, _, format)| format.is_model())
            .count();
        let folder = self.intern(relative);
        for (path, metadata, format) in files {
            self.files += 1;
            if self.files > MOST_FILES {
                if self.files == MOST_FILES + 1 {
                    tracing::warn!(path = %dir.display(), "library holds too many files, rest skipped");
                }
                return Ok(());
            }
            let lone_model = shelf.holds_models() && format.is_model() && models == 1;
            let sidecar = sidecar(path, &names).or_else(|| match lone_model {
                true => folder_picture(&names),
                false => None,
            });
            self.found.push(Found {
                path: path.clone(),
                shelf,
                folder: Arc::clone(&folder),
                format,
                stamp: Stamp {
                    size: metadata.len(),
                    modified: metadata.modified()?,
                },
                sidecar,
            });
        }
        if depth >= DEEPEST {
            return Ok(());
        }
        for (path, metadata) in &children {
            if !metadata.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let nested = match relative.is_empty() {
                true => name.to_owned(),
                false => format!("{relative}/{name}"),
            };
            if let Err(error) = self.folder(path, shelf, &nested, depth + 1) {
                tracing::debug!(path = %path.display(), %error, "failed to scan library folder");
            }
        }
        Ok(())
    }

    fn intern(&mut self, relative: &str) -> Arc<str> {
        Arc::clone(
            self.folders
                .entry(relative.to_owned())
                .or_insert_with(|| Arc::from(relative)),
        )
    }
}

fn listing(dir: &Path) -> io::Result<Vec<(PathBuf, Metadata)>> {
    let mut children = Vec::new();
    for child in std::fs::read_dir(dir)? {
        let child = child?;
        let hidden = child
            .file_name()
            .to_str()
            .is_none_or(|name| name.starts_with('.'));
        if hidden {
            continue;
        }
        let path = child.path();
        let metadata = match child.file_type()?.is_symlink() {
            true => std::fs::metadata(&path),
            false => child.metadata(),
        };
        match metadata {
            Ok(metadata) => children.push((path, metadata)),
            Err(error) => tracing::debug!(path = %path.display(), %error, "skipped library entry"),
        }
    }
    Ok(children)
}

fn sidecar(path: &Path, names: &HashMap<String, &Path>) -> Option<PathBuf> {
    let stem = path.file_stem()?.to_str()?.to_lowercase();
    PICTURES
        .iter()
        .find_map(|extension| names.get(&format!("{stem}.{extension}")))
        .map(|picture| picture.to_path_buf())
}

fn folder_picture(names: &HashMap<String, &Path>) -> Option<PathBuf> {
    FOLDER_PICTURES
        .iter()
        .find_map(|name| names.get(*name))
        .map(|picture| picture.to_path_buf())
}
