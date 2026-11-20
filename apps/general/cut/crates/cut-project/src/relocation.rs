use std::cmp::Reverse;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

const DEPTH: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relocation {
    from: PathBuf,
    to: PathBuf,
    beside: PathBuf,
}

impl Relocation {
    #[must_use]
    pub fn between(original: &Path, found: &Path) -> Option<Relocation> {
        let (old, new) = (original.parent()?, found.parent()?);
        let old_parts: Vec<Component<'_>> = old.components().collect();
        let new_parts: Vec<Component<'_>> = new.components().collect();
        let shared = shared_tail(&old_parts, &new_parts);
        let prefix =
            |parts: &[Component<'_>]| parts[..parts.len() - shared].iter().collect::<PathBuf>();
        Some(Relocation {
            from: prefix(&old_parts),
            to: prefix(&new_parts),
            beside: new.to_path_buf(),
        })
    }

    #[must_use]
    pub fn candidates(&self, original: &Path) -> Vec<PathBuf> {
        let rebased = original
            .strip_prefix(&self.from)
            .ok()
            .map(|rest| self.to.join(rest));
        let beside = original.file_name().map(|name| self.beside.join(name));
        let mut candidates: Vec<PathBuf> = rebased.into_iter().collect();
        if let Some(beside) = beside
            && !candidates.contains(&beside)
        {
            candidates.push(beside);
        }
        candidates
    }
}

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    files: HashMap<String, Vec<PathBuf>>,
    unreadable: usize,
}

impl Catalog {
    pub fn scan<'a>(
        root: &Path,
        wanted: impl IntoIterator<Item = &'a Path>,
    ) -> io::Result<Catalog> {
        let mut catalog = Catalog {
            files: wanted
                .into_iter()
                .filter_map(Path::file_name)
                .map(|name| (folded(name), Vec::new()))
                .collect(),
            unreadable: 0,
        };
        let mut folders = Vec::new();
        catalog.take(fs::read_dir(root)?, 0, &mut folders);
        while let Some((folder, depth)) = folders.pop() {
            match fs::read_dir(&folder) {
                Ok(entries) => catalog.take(entries, depth, &mut folders),
                Err(_) => catalog.unreadable += 1,
            }
        }
        Ok(catalog)
    }

    fn take(&mut self, entries: fs::ReadDir, depth: usize, folders: &mut Vec<(PathBuf, usize)>) {
        for entry in entries {
            let Some((entry, kind)) = entry
                .ok()
                .and_then(|entry| entry.file_type().ok().map(|kind| (entry, kind)))
            else {
                self.unreadable += 1;
                continue;
            };
            let name = entry.file_name();
            if kind.is_dir() {
                if depth < DEPTH && !is_hidden(&name) {
                    folders.push((entry.path(), depth + 1));
                }
                continue;
            }
            let Some(found) = self.files.get_mut(&folded(&name)) else {
                continue;
            };
            let path = entry.path();
            if kind.is_file() || (kind.is_symlink() && path.is_file()) {
                found.push(path);
            }
        }
    }

    #[must_use]
    pub fn unreadable(&self) -> usize {
        self.unreadable
    }

    #[must_use]
    pub fn find(&self, original: &Path) -> Vec<PathBuf> {
        let Some(name) = original.file_name() else {
            return Vec::new();
        };
        let mut found = self.files.get(&folded(name)).cloned().unwrap_or_default();
        let folders = |path: &Path| -> Vec<String> {
            path.parent()
                .into_iter()
                .flat_map(Path::components)
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect()
        };
        let wanted = folders(original);
        found.sort_by_cached_key(|path| {
            (
                Reverse(shared_tail(&wanted, &folders(path))),
                path.file_name() != Some(name),
                path.clone(),
            )
        });
        found
    }
}

fn shared_tail<T: PartialEq>(one: &[T], other: &[T]) -> usize {
    one.iter()
        .rev()
        .zip(other.iter().rev())
        .take_while(|(a, b)| a == b)
        .count()
        .min(one.len().saturating_sub(1))
        .min(other.len().saturating_sub(1))
}

fn is_hidden(name: &OsStr) -> bool {
    name.as_encoded_bytes().first() == Some(&b'.')
}

fn folded(name: &OsStr) -> String {
    name.to_string_lossy().to_lowercase()
}
