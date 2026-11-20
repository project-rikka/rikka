use std::collections::HashSet;
use std::fs::{FileType, Metadata};
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub(crate) struct Walk {
    pub(crate) folders: Vec<PathBuf>,
    pub(crate) files: Vec<PathBuf>,
}

pub(crate) fn walk(root: &Path) -> Walk {
    let mut seen = HashSet::new();
    let mut walk = Walk::default();
    let mut waiting = vec![root.to_path_buf()];
    while let Some(folder) = waiting.pop() {
        let Ok(metadata) = std::fs::metadata(&folder) else {
            continue;
        };
        if !metadata.is_dir() {
            walk.files.push(folder);
            continue;
        }
        if !seen.insert(identity(&metadata, &folder)) {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&folder) {
            for entry in entries.flatten() {
                match entry.file_type() {
                    Ok(kind) if descends(kind) => waiting.push(entry.path()),
                    Ok(_) => walk.files.push(entry.path()),
                    Err(_) => {}
                }
            }
        }
        walk.folders.push(folder);
    }
    walk
}

#[cfg(unix)]
fn identity(metadata: &Metadata, _folder: &Path) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt as _;
    (metadata.dev(), metadata.ino())
}

#[cfg(unix)]
fn descends(kind: FileType) -> bool {
    kind.is_dir() || kind.is_symlink()
}

#[cfg(windows)]
fn identity(_metadata: &Metadata, folder: &Path) -> PathBuf {
    folder.to_path_buf()
}

#[cfg(windows)]
fn descends(kind: FileType) -> bool {
    kind.is_dir()
}
