use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rikka_dirs::data_dirs;

use crate::entry::{DesktopEntry, desktops};
use crate::locale::locales;

const MAX_DEPTH: usize = 4;

pub(crate) fn application_dirs() -> Vec<PathBuf> {
    data_dirs()
        .into_iter()
        .map(|dir| dir.join("applications"))
        .collect()
}

#[derive(Default)]
pub struct Catalog {
    entries: Vec<DesktopEntry>,
    listable: Vec<bool>,
    sources: Sources,
}

impl Catalog {
    #[must_use]
    pub fn read() -> Self {
        let locales = locales();
        let directories = application_dirs();

        let mut entries: Vec<DesktopEntry> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for directory in &directories {
            for (id, path) in desktop_files(directory) {
                if !seen.insert(id.clone()) {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                if let Some(entry) = DesktopEntry::parse(&id, &path, &text, &locales) {
                    entries.push(entry);
                }
            }
        }

        let here = desktops();
        let listable = entries
            .iter()
            .map(|entry| !entry.no_display && entry.shows_in(&here) && entry.is_installed())
            .collect();

        let sources = Sources::from_dirs(&directories);
        tracing::debug!(count = entries.len(), "loaded desktop entries");
        Self {
            entries,
            listable,
            sources,
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[DesktopEntry] {
        &self.entries
    }

    pub fn into_listable(self) -> Vec<DesktopEntry> {
        self.entries
            .into_iter()
            .zip(self.listable)
            .filter(|(_, listable)| *listable)
            .map(|(entry, _)| entry)
            .collect()
    }

    pub fn listable(&self) -> impl Iterator<Item = &DesktopEntry> {
        self.entries
            .iter()
            .zip(&self.listable)
            .filter(|(_, listable)| **listable)
            .map(|(entry, _)| entry)
    }

    #[must_use]
    pub fn is_stale(&self) -> bool {
        self.sources.is_stale(&application_dirs())
    }
}

impl std::fmt::Debug for Catalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Catalog")
            .field("entries", &self.entries.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Sources(Vec<Source>);

impl Sources {
    #[must_use]
    pub fn from_dirs(directories: &[PathBuf]) -> Self {
        Self(
            directories
                .iter()
                .map(|path| Source::from_path(path))
                .collect(),
        )
    }

    #[must_use]
    pub fn is_stale(&self, now: &[PathBuf]) -> bool {
        if now.len() != self.0.len() {
            return true;
        }
        self.0
            .iter()
            .zip(now)
            .any(|(was, now)| *was != Source::from_path(now))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Source {
    named: PathBuf,
    leads_to: Option<PathBuf>,
    modified: Option<SystemTime>,
}

impl Source {
    fn from_path(named: &Path) -> Source {
        Source {
            named: named.to_path_buf(),
            leads_to: std::fs::canonicalize(named).ok(),
            modified: modified(named),
        }
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

pub(crate) fn desktop_files(root: &Path) -> Vec<(String, PathBuf)> {
    let mut found = Vec::new();
    collect(root, root, 0, &mut found);
    found
}

fn collect(root: &Path, directory: &Path, depth: usize, out: &mut Vec<(String, PathBuf)>) {
    let Ok(listing) = std::fs::read_dir(directory) else {
        return;
    };
    for item in listing.flatten() {
        let path = item.path();
        let Ok(kind) = item.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if depth < MAX_DEPTH {
                collect(root, &path, depth + 1, out);
            }
            continue;
        }
        if path
            .extension()
            .is_none_or(|extension| extension != "desktop")
        {
            continue;
        }
        if let Some(id) = identifier(root, &path) {
            out.push((id, path));
        }
    }
}

fn identifier(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    Some(relative.to_str()?.replace('/', "-"))
}
