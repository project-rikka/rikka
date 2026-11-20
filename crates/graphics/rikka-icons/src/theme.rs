use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use rikka_xdg::KeyFile;

use crate::directory::Directory;
use crate::keys;

const INDEX: &str = "index.theme";

const GROUP: &str = "Icon Theme";

pub(crate) const EXTENSIONS: [&str; 3] = ["svg", "svgz", "png"];

#[derive(Debug)]
pub(crate) struct Theme {
    inherits: Vec<String>,
    directories: Vec<Directory>,
    roots: Vec<PathBuf>,
    index: OnceLock<Index>,
}

type Index = HashMap<Box<str>, Vec<Entry>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Entry {
    directory: u32,
    root: u32,
    extension: u8,
}

impl Theme {
    pub(crate) fn read(name: &str, bases: &[PathBuf]) -> Option<Theme> {
        let mut indexed = false;
        let mut inherits: Vec<String> = Vec::new();
        let mut directories: Vec<Directory> = Vec::new();
        let mut roots: Vec<PathBuf> = Vec::new();

        for base in bases {
            let root = base.join(name);
            let Ok(text) = std::fs::read_to_string(root.join(INDEX)) else {
                if root.is_dir() {
                    roots.push(root);
                }
                continue;
            };
            roots.push(root);
            indexed = true;

            let file = KeyFile::parse(&text);
            let Some(theme) = file.group(GROUP) else {
                continue;
            };
            if inherits.is_empty() {
                inherits = keys::list(theme, "Inherits")
                    .into_iter()
                    .map(str::to_owned)
                    .collect();
            }
            for path in keys::list(theme, "Directories")
                .into_iter()
                .chain(keys::list(theme, "ScaledDirectories"))
            {
                if directories.iter().any(|listed| listed.path == path) {
                    continue;
                }
                let Some(group) = file.group(path) else {
                    continue;
                };
                directories.extend(Directory::read(path, group));
            }
        }

        indexed.then(|| Theme {
            inherits,
            directories,
            roots,
            index: OnceLock::new(),
        })
    }

    pub(crate) fn inherits(&self) -> &[String] {
        &self.inherits
    }

    pub(crate) fn clear_cache(&mut self) {
        self.index.take();
    }

    pub(crate) fn lookup(&self, name: &str, size: u32, scale: u32) -> Option<Arc<Path>> {
        let index = self.index.get_or_init(|| self.scan()).get(name)?;
        let mut closest: Option<(u32, Arc<Path>)> = None;
        let mut checked = None;
        for entry in index {
            if checked == Some(entry.directory) {
                continue;
            }
            let path = self.path(*entry, name);
            if !path.is_file() {
                continue;
            }
            checked = Some(entry.directory);
            let directory = &self.directories[entry.directory as usize];
            let icon = Arc::from(path);
            if directory.matches(size, scale) {
                return Some(icon);
            }
            let distance = directory.distance(size, scale);
            if closest
                .as_ref()
                .is_none_or(|(nearest, _)| distance < *nearest)
            {
                closest = Some((distance, icon));
            }
        }
        closest.map(|(_, icon)| icon)
    }

    fn path(&self, entry: Entry, name: &str) -> PathBuf {
        let extension = EXTENSIONS[usize::from(entry.extension)];
        self.roots[entry.root as usize]
            .join(&self.directories[entry.directory as usize].path)
            .join(format!("{name}.{extension}"))
    }

    fn scan(&self) -> Index {
        let mut index = Index::new();
        for (directory, listed) in self.directories.iter().enumerate() {
            for (root, root_path) in self.roots.iter().enumerate() {
                let folder = root_path.join(&listed.path);
                let Ok(entries) = std::fs::read_dir(folder) else {
                    continue;
                };
                for entry in entries.filter_map(Result::ok) {
                    let file_name = entry.file_name();
                    let Some((stem, extension)) =
                        file_name.to_str().and_then(|name| name.rsplit_once('.'))
                    else {
                        continue;
                    };
                    let Some(extension) = EXTENSIONS.iter().position(|listed| *listed == extension)
                    else {
                        continue;
                    };
                    index.entry(Box::from(stem)).or_default().push(Entry {
                        directory: directory as u32,
                        root: root as u32,
                        extension: extension as u8,
                    });
                }
            }
        }
        for entries in index.values_mut() {
            entries.sort_unstable();
            entries.shrink_to_fit();
        }
        index.shrink_to_fit();
        index
    }
}

pub(crate) fn lookup_pixmap(roots: &[PathBuf], name: &str) -> Option<Arc<Path>> {
    for root in roots {
        for extension in EXTENSIONS {
            let path = root.join(format!("{name}.{extension}"));
            if path.is_file() {
                return Some(Arc::from(path));
            }
        }
    }
    None
}
