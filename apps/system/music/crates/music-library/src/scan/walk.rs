use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::scan::halt::Halt;

use crate::index::Stat;
use crate::paths;

const AUDIO: [&str; 28] = [
    "flac", "mp3", "mp2", "mp1", "m4a", "m4b", "mp4", "aac", "alac", "wav", "wave", "aif", "aiff",
    "aifc", "ape", "wv", "opus", "ogg", "oga", "spx", "mpc", "mp+", "mpp", "dsf", "dff", "tak",
    "tta", "wma",
];

pub(crate) fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            AUDIO
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
}

#[derive(Debug, Default)]
pub(crate) struct Walk {
    folders: HashSet<(u64, u64)>,
    files: HashSet<(u64, u64)>,
    pub(crate) found: Vec<Stat>,
}

impl Walk {
    pub(crate) fn excluding(excluded: &[PathBuf]) -> Walk {
        let folders = excluded
            .iter()
            .filter_map(|path| fs::metadata(path).ok())
            .map(|metadata| (metadata.dev(), metadata.ino()))
            .collect();
        Walk {
            folders,
            ..Walk::default()
        }
    }

    pub(crate) fn walk(&mut self, start: &Path, halt: Halt<'_>) {
        let mut pending = vec![start.to_path_buf()];
        while let Some(folder) = pending.pop() {
            if halt.is_set() {
                return;
            }
            let Ok(metadata) = fs::metadata(&folder) else {
                continue;
            };
            if !metadata.is_dir() || !self.folders.insert((metadata.dev(), metadata.ino())) {
                continue;
            }
            let Ok(entries) = fs::read_dir(&folder) else {
                continue;
            };
            for entry in entries.filter_map(Result::ok) {
                if paths::is_hidden(&entry.file_name()) {
                    continue;
                }
                let path = entry.path();
                let Ok(metadata) = fs::metadata(&path) else {
                    continue;
                };
                if metadata.is_dir() {
                    pending.push(path);
                } else if metadata.is_file()
                    && is_audio(&path)
                    && self.files.insert((metadata.dev(), metadata.ino()))
                {
                    self.found.push(Stat::from_metadata(path, &metadata));
                }
            }
        }
    }
}
