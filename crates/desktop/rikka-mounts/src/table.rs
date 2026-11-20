use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::mount::Mount;

const MOUNTINFO: &str = "/proc/self/mountinfo";

#[must_use]
pub fn mounts() -> Vec<Mount> {
    std::fs::read_to_string(MOUNTINFO)
        .unwrap_or_default()
        .lines()
        .filter_map(Mount::parse)
        .collect()
}

#[must_use]
pub fn containing(path: &Path) -> Option<Mount> {
    mounts()
        .into_iter()
        .filter(|mount| path.starts_with(&mount.at))
        .max_by_key(|mount| mount.at.components().count())
}

#[must_use]
pub fn top_directories() -> Vec<PathBuf> {
    let mut tops: Vec<PathBuf> = mounts()
        .into_iter()
        .filter(Mount::is_storage)
        .map(|mount| mount.at)
        .collect();
    tops.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    let mut seen = HashSet::new();
    tops.retain(|path| seen.insert(path.clone()));
    tops
}
