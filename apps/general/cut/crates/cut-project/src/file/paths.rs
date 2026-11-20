use std::fs;
use std::path::{Component, Path, PathBuf};

pub(super) fn relative(path: &Path, folder: &Path) -> Option<PathBuf> {
    if !path.is_absolute() || !folder.is_absolute() {
        return None;
    }
    let target: Vec<Component<'_>> = path.components().collect();
    let base: Vec<Component<'_>> = folder.components().collect();
    let shared = target
        .iter()
        .zip(&base)
        .take_while(|(one, other)| one == other)
        .count();
    let mut relative: PathBuf = base[shared..]
        .iter()
        .map(|_| Component::ParentDir)
        .collect();
    relative.extend(&target[shared..]);
    Some(relative)
}

pub(super) fn located(recorded: &Path, relative: Option<&Path>, folder: &Path) -> PathBuf {
    let recorded = match recorded.is_absolute() {
        true => recorded.to_path_buf(),
        false => folder.join(recorded),
    };
    if recorded.is_file() {
        return recorded;
    }
    relative
        .map(|relative| folder.join(relative))
        .filter(|beside| beside.is_file())
        .and_then(|beside| fs::canonicalize(beside).ok())
        .unwrap_or(recorded)
}
