use std::path::{Path, PathBuf};

const MARKDOWN: &str = "md";

#[must_use]
pub fn markdown_files(folder: &Path) -> Vec<PathBuf> {
    listed(folder, |path| {
        let shown = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| !name.starts_with('.'));
        let markdown = path
            .extension()
            .is_some_and(|kind| kind.eq_ignore_ascii_case(MARKDOWN));
        shown && markdown && path.is_file()
    })
}

#[must_use]
pub fn folders(folder: &Path) -> Vec<PathBuf> {
    listed(folder, |path| {
        path.symlink_metadata().is_ok_and(|about| about.is_dir())
    })
}

fn listed(folder: &Path, keep: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };

    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| keep(path))
        .collect();
    paths.sort();
    paths
}
