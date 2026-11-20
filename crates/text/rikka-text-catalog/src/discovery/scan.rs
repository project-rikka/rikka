use std::collections::HashSet;
use std::fs::Metadata;
use std::path::{Path, PathBuf};

use crate::config::Fontconfig;
use crate::discovery::stamp::FileStamp;

const FONT_EXTENSIONS: [&str; 4] = ["ttf", "otf", "ttc", "otc"];

#[derive(Clone, Debug)]
pub(crate) struct FontPath {
    pub(crate) path: PathBuf,
    pub(crate) stamp: FileStamp,
}

pub(crate) fn font_files(
    files: &[PathBuf],
    dirs: &[PathBuf],
    config: &Fontconfig,
) -> Vec<FontPath> {
    let mut walk = Walk {
        config,
        visited_dirs: HashSet::new(),
        seen_files: HashSet::new(),
        found: Vec::new(),
    };
    for file in files {
        if let Ok(metadata) = std::fs::metadata(file)
            && metadata.is_file()
        {
            walk.file(file.clone(), &metadata);
        }
    }
    for dir in dirs {
        walk.dir(dir);
    }
    walk.found
}

struct Walk<'a> {
    config: &'a Fontconfig,
    visited_dirs: HashSet<(u64, u64)>,
    seen_files: HashSet<(u64, u64)>,
    found: Vec<FontPath>,
}

impl Walk<'_> {
    fn dir(&mut self, dir: &Path) {
        let Ok(metadata) = std::fs::metadata(dir) else {
            return;
        };
        if !metadata.is_dir() {
            return;
        }
        let Ok(stamp) = FileStamp::of_path(dir, &metadata) else {
            return;
        };
        if !self.visited_dirs.insert(stamp.identity()) {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if file_type.is_dir() {
                self.dir(&path);
                continue;
            }
            if file_type.is_symlink() {
                match std::fs::metadata(&path) {
                    Ok(target) if target.is_dir() => {
                        self.dir(&path);
                        continue;
                    }
                    Ok(target) if target.is_file() => self.file(path, &target),
                    _ => {}
                }
            } else if file_type.is_file()
                && let Ok(metadata) = entry.metadata()
            {
                self.file(path, &metadata);
            }
        }
    }

    fn file(&mut self, path: PathBuf, metadata: &Metadata) {
        if !has_font_extension(&path) || !self.config.accepts(&path) {
            return;
        }
        let Ok(stamp) = FileStamp::of_path(&path, metadata) else {
            return;
        };
        if self.seen_files.insert(stamp.identity()) {
            self.found.push(FontPath { path, stamp });
        }
    }
}

fn has_font_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            FONT_EXTENSIONS
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}
