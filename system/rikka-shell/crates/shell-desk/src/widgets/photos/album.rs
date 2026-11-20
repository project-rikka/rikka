use std::cmp::Ordering;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use rikka_dirs::{UserDir, UserDirs};
use rikka_format::natural_cmp;
use rikka_media_format::{Format, Kind};

const MAX_DEPTH: usize = 3;
const MAX_SCANNED_ENTRIES: usize = 20_000;
const MAX_PICTURES: usize = 4_000;

#[derive(Debug, Default)]
pub(crate) struct Album {
    pub(crate) folder: Option<PathBuf>,
    pub(crate) found: bool,
    pub(crate) pictures: Vec<PathBuf>,
}

#[must_use]
pub(super) fn scan_album(chosen: Option<&str>) -> Album {
    let folder = match chosen {
        Some(chosen) => Some(PathBuf::from(chosen)),
        None => UserDirs::load().path(UserDir::Pictures),
    };
    let Some(folder) = folder.filter(|folder| folder.is_dir()) else {
        return Album {
            folder: chosen.map(PathBuf::from),
            ..Album::default()
        };
    };
    let mut pictures = pictures_in(&folder);
    pictures.sort_by(|left, right| natural_path_cmp(left, right));
    Album {
        folder: Some(folder),
        found: true,
        pictures,
    }
}

fn pictures_in(folder: &Path) -> Vec<PathBuf> {
    let mut pictures = Vec::new();
    let mut scanned = 0;
    let mut queue = VecDeque::from([(folder.to_path_buf(), 0)]);
    while let Some((directory, depth)) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            scanned += 1;
            if scanned > MAX_SCANNED_ENTRIES || pictures.len() >= MAX_PICTURES {
                return pictures;
            }
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if depth < MAX_DEPTH {
                    queue.push_back((path, depth + 1));
                }
            } else if is_picture(&path) {
                pictures.push(path);
            }
        }
    }
    pictures
}

fn is_picture(path: &Path) -> bool {
    Format::from_path(path).is_some_and(|format| format.kind() == Kind::Picture)
}

fn natural_path_cmp(left: &Path, right: &Path) -> Ordering {
    natural_cmp(&left.to_string_lossy(), &right.to_string_lossy())
}
