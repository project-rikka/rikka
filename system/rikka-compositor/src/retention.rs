use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub(crate) fn keep_newest(directory: &Path, count: usize) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            return tracing::warn!("failed to list {}: {error}", directory.display());
        }
    };
    let mut logs: Vec<(SystemTime, PathBuf)> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "log"))
        .filter_map(|path| Some((fs::metadata(&path).ok()?.modified().ok()?, path)))
        .collect();
    logs.sort_unstable_by_key(|(modified, _)| Reverse(*modified));
    for (_, path) in logs.into_iter().skip(count) {
        if let Err(error) = fs::remove_file(&path) {
            tracing::warn!("failed to remove {}: {error}", path.display());
        }
    }
}
