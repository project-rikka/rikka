use std::path::{Path, PathBuf};

use rikka_xdg::{RecentApplication, RecentFile, RecentFiles};

const PAINT: RecentApplication = RecentApplication::new("rikka-paint", "rikka-paint %u");
const SHOWN: usize = 12;

pub(crate) fn load() -> Vec<RecentFile> {
    let Some(recent) = RecentFiles::user() else {
        return Vec::new();
    };
    match recent.for_application(&PAINT, SHOWN) {
        Ok(files) => files,
        Err(error) => {
            tracing::warn!(%error, "failed to read recently used files");
            Vec::new()
        }
    }
}

pub(crate) fn note(path: &Path, mime_type: &str) -> Vec<RecentFile> {
    let Some(recent) = RecentFiles::user() else {
        return Vec::new();
    };
    if let Err(error) = recent.add(path, mime_type, &PAINT) {
        tracing::warn!(%error, path = %path.display(), "failed to add recently used file");
    }
    load()
}

pub(crate) fn folder_of(files: &[RecentFile]) -> Option<PathBuf> {
    files.first()?.path.parent().map(Path::to_path_buf)
}
