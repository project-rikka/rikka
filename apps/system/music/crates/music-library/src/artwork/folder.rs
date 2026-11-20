use std::collections::HashMap;
use std::fs::{self, DirEntry};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::paths;

pub(crate) const IMAGE_EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];
const NAMES: [&str; 4] = ["cover", "folder", "front", "album"];
const PREFIX: &str = "albumart";
const SETTLED: Duration = Duration::from_secs(2);

#[derive(Debug, Default)]
pub(crate) struct FolderImages {
    cached: HashMap<PathBuf, (SystemTime, Option<PathBuf>)>,
}

impl FolderImages {
    pub(crate) fn image(&mut self, folder: &Path) -> Option<PathBuf> {
        let Some(modified) = fs::metadata(folder)
            .and_then(|metadata| metadata.modified())
            .ok()
        else {
            self.cached.remove(folder);
            return None;
        };
        if let Some((stamp, image)) = self.cached.get(folder)
            && *stamp == modified
        {
            return image.clone();
        }
        let image = listed(folder);
        let is_stable = SystemTime::now()
            .duration_since(modified)
            .is_ok_and(|age| age >= SETTLED);
        if is_stable {
            self.cached
                .insert(folder.to_path_buf(), (modified, image.clone()));
        } else {
            self.cached.remove(folder);
        }
        image
    }
}

fn listed(folder: &Path) -> Option<PathBuf> {
    let mut images: Vec<(usize, String, PathBuf)> = fs::read_dir(folder)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| !paths::is_hidden(&entry.file_name()))
        .filter(is_file)
        .filter_map(|entry| {
            let path = entry.path();
            let extension = path.extension()?.to_str()?.to_ascii_lowercase();
            if !IMAGE_EXTENSIONS.contains(&extension.as_str()) {
                return None;
            }
            let stem = path.file_stem()?.to_str()?.to_lowercase();
            let rank = NAMES
                .iter()
                .position(|name| stem == *name)
                .or_else(|| stem.starts_with(PREFIX).then_some(NAMES.len()))
                .unwrap_or(usize::MAX);
            Some((rank, stem, path))
        })
        .collect();
    images.sort();
    let lone = images.len() == 1;
    images
        .into_iter()
        .next()
        .filter(|(rank, _, _)| *rank != usize::MAX || lone)
        .map(|(_, _, path)| path)
}

fn is_file(entry: &DirEntry) -> bool {
    match entry.file_type() {
        Ok(kind) if kind.is_symlink() => entry.path().is_file(),
        Ok(kind) => kind.is_file(),
        Err(_) => false,
    }
}
