use std::collections::{HashMap, HashSet};
use std::io;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use rikka_proton_drive_sync_state::{Changes, Kind, LocalItem, is_ignored};

use crate::local::files::{file_sha1, modified};

#[derive(Debug, Default)]
pub(crate) struct Scan {
    pub(crate) changes: Changes,
    pub(crate) invalid: Vec<PathBuf>,
    pub(crate) unreadable: Vec<PathBuf>,
}

pub(crate) struct Scanner<'a> {
    root: &'a Path,
    root_inode: u64,
    excluded: &'a [PathBuf],
    known: &'a HashMap<u64, LocalItem>,
    children: HashMap<u64, Vec<u64>>,
}

struct Walk<'a> {
    scanner: &'a Scanner<'a>,
    device: u64,
    found: HashMap<u64, LocalItem>,
    invalid: Vec<PathBuf>,
    unreadable: Vec<PathBuf>,
}

impl<'a> Scanner<'a> {
    pub(crate) fn new(
        root: &'a Path,
        root_inode: u64,
        excluded: &'a [PathBuf],
        known: &'a HashMap<u64, LocalItem>,
    ) -> Scanner<'a> {
        let mut children: HashMap<u64, Vec<u64>> = HashMap::new();
        for item in known.values() {
            children.entry(item.parent).or_default().push(item.id);
        }
        Scanner {
            root,
            root_inode,
            excluded,
            known,
            children,
        }
    }

    fn children(&self, id: u64) -> &[u64] {
        self.children
            .get(&id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(crate) fn full(&self) -> io::Result<Scan> {
        let mut walk = self.walk()?;
        walk.list(self.root, self.root_inode, true);
        let removed = self
            .known
            .keys()
            .filter(|id| !walk.found.contains_key(id))
            .copied()
            .collect();
        Ok(walk.finish(removed))
    }

    pub(crate) fn folders(&self, folders: &HashSet<PathBuf>) -> io::Result<Scan> {
        let mut walk = self.walk()?;
        let mut missing = Vec::new();
        for folder in folders {
            let Some(inode) = self.known_folder(folder) else {
                continue;
            };
            let listed: HashSet<u64> = walk.list(folder, inode, false).into_iter().collect();
            missing.extend(
                self.children(inode)
                    .iter()
                    .filter(|id| !listed.contains(id))
                    .copied(),
            );
        }

        let mut removed = Vec::new();
        let mut visited: HashSet<u64> = HashSet::new();
        let mut next = 0;
        while next < missing.len() {
            let id = missing[next];
            next += 1;
            if walk.found.contains_key(&id) || !visited.insert(id) {
                continue;
            }
            removed.push(id);
            missing.extend(self.children(id).iter().copied());
        }
        Ok(walk.finish(removed))
    }

    fn known_folder(&self, folder: &Path) -> Option<u64> {
        if folder == self.root {
            return Some(self.root_inode);
        }
        let metadata = std::fs::symlink_metadata(folder).ok()?;
        let inode = metadata.ino();
        self.known
            .get(&inode)
            .is_some_and(|item| item.kind == Kind::Folder && metadata.is_dir())
            .then_some(inode)
    }

    fn walk(&self) -> io::Result<Walk<'_>> {
        let device = std::fs::symlink_metadata(self.root)?.dev();
        Ok(Walk {
            scanner: self,
            device,
            found: HashMap::new(),
            invalid: Vec::new(),
            unreadable: Vec::new(),
        })
    }
}

impl Walk<'_> {
    fn list(&mut self, folder: &Path, inode: u64, everything: bool) -> Vec<u64> {
        let Ok(entries) = std::fs::read_dir(folder) else {
            self.unreadable.push(folder.to_owned());
            return self.retain_known(inode);
        };
        let mut listed = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                self.invalid.push(path);
                continue;
            };
            if is_ignored(&name) || self.scanner.excluded.contains(&path) {
                continue;
            }
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            let kind = match metadata.file_type() {
                kind if kind.is_dir() => Kind::Folder,
                kind if kind.is_file() => Kind::File,
                _ => continue,
            };
            if kind == Kind::Folder && metadata.dev() != self.device {
                continue;
            }
            let id = metadata.ino();
            let item = self.item(&path, id, inode, name, kind, &metadata);
            listed.push(id);
            let descend = kind == Kind::Folder
                && (everything
                    || !self
                        .scanner
                        .known
                        .get(&id)
                        .is_some_and(|known| known.kind == Kind::Folder));
            self.found.insert(id, item);
            if descend {
                self.list(&path, id, everything);
            }
        }
        listed
    }

    fn retain_known(&mut self, inode: u64) -> Vec<u64> {
        let scanner = self.scanner;
        let children = scanner.children(inode).to_vec();
        let mut retained = children.clone();
        let mut next = 0;
        while next < retained.len() {
            let id = retained[next];
            next += 1;
            if let Some(item) = scanner.known.get(&id) {
                self.found.insert(id, item.clone());
                retained.extend(scanner.children(id).iter().copied());
            }
        }
        children
    }

    fn item(
        &mut self,
        path: &Path,
        id: u64,
        parent: u64,
        name: String,
        kind: Kind,
        metadata: &std::fs::Metadata,
    ) -> LocalItem {
        let modified = modified(metadata);
        let size = match kind {
            Kind::File => metadata.len(),
            Kind::Folder => 0,
        };
        let sha1 = match kind {
            Kind::Folder => None,
            Kind::File => {
                let cached = self.scanner.known.get(&id).filter(|known| {
                    known.kind == Kind::File && known.size == size && known.modified == modified
                });
                match cached.and_then(|known| known.sha1.clone()) {
                    Some(sha1) => Some(sha1),
                    None => match file_sha1(path) {
                        Ok(sha1) => Some(sha1),
                        Err(_) => {
                            self.unreadable.push(path.to_owned());
                            None
                        }
                    },
                }
            }
        };
        LocalItem {
            id,
            parent,
            name,
            kind,
            size,
            modified,
            sha1,
        }
    }

    fn finish(self, removed: Vec<u64>) -> Scan {
        let known = self.scanner.known;
        let local = self
            .found
            .into_values()
            .filter(|item| known.get(&item.id) != Some(item))
            .collect();
        Scan {
            changes: Changes {
                local,
                local_removed: removed,
                ..Changes::default()
            },
            invalid: self.invalid,
            unreadable: self.unreadable,
        }
    }
}
