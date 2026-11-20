use std::path::PathBuf;

use rikka_dirs::{data_dirs, home};

pub(crate) fn theme_roots() -> Vec<PathBuf> {
    home()
        .map(|home| home.join(".icons"))
        .into_iter()
        .chain(data_dirs().into_iter().map(|dir| dir.join("icons")))
        .collect()
}

pub(crate) fn pixmap_roots() -> Vec<PathBuf> {
    let system_pixmaps = PathBuf::from("/run/current-system/sw/share/pixmaps");
    let mut roots: Vec<PathBuf> = data_dirs()
        .into_iter()
        .map(|dir| dir.join("pixmaps"))
        .collect();
    if !roots.contains(&system_pixmaps) {
        roots.push(system_pixmaps);
    }
    roots
}
