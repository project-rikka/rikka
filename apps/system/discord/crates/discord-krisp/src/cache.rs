use std::path::{Path, PathBuf};

use crate::module::{Module, version};

impl Module {
    #[must_use]
    pub fn cached(cache: &Path) -> Option<Module> {
        releases(cache)
            .into_iter()
            .find_map(|folder| Module::from_folder(folder).ok())
    }

    pub fn prune_others(&self, cache: &Path) {
        for folder in releases(cache) {
            let real_folder = folder
                .symlink_metadata()
                .is_ok_and(|metadata| metadata.is_dir());
            if real_folder && folder != self.folder() {
                let _ = std::fs::remove_dir_all(&folder);
            }
        }
        for leftover in leftovers(cache) {
            let _ = std::fs::remove_dir_all(&leftover).or_else(|_| std::fs::remove_file(&leftover));
        }
    }
}

fn leftovers(cache: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(cache) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let hidden = entry.file_name().to_str()?.starts_with('.');
            hidden.then(|| entry.path())
        })
        .collect()
}

fn releases(cache: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(cache) else {
        return Vec::new();
    };
    let mut releases: Vec<(Vec<u64>, PathBuf)> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            let numbered = name.starts_with(|character: char| character.is_ascii_digit());
            numbered.then(|| (version(&name), entry.path()))
        })
        .collect();
    releases.sort_by(|a, b| b.0.cmp(&a.0));
    releases.into_iter().map(|(_, folder)| folder).collect()
}
