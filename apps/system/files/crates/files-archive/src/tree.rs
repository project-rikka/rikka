use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use rikka_archive::{Archive, EntryKind};

use crate::error::ArchiveError;

#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub(crate) name: String,
    pub(crate) directory: bool,
    pub(crate) index: Option<usize>,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) mode: Option<u32>,
}

#[derive(Debug, Default)]
pub(crate) struct Tree {
    inside: HashMap<PathBuf, Vec<Node>>,
}

impl Tree {
    pub(crate) fn read(archive: &Path, password: Option<&str>) -> Result<Self, ArchiveError> {
        let opened = match password {
            Some(password) => Archive::open_with_password(archive, password),
            None => Archive::open(archive),
        }
        .map_err(|error| ArchiveError::from_archive(&error))?;

        let mut building: HashMap<PathBuf, HashMap<String, Node>> = HashMap::new();
        building.insert(PathBuf::new(), HashMap::new());

        for (index, entry) in opened.entries().iter().enumerate() {
            let Some(path) = inside(entry.name()) else {
                continue;
            };
            let directory = entry.kind() == EntryKind::Directory;
            let leaf = Node {
                name: crate::format::file_name(&path),
                directory,
                index: Some(index),
                size: if directory { 0 } else { entry.size() },
                modified: entry.modified(),
                mode: entry.mode(),
            };
            claim(&mut building, &path, leaf);
        }

        Ok(Self {
            inside: building
                .into_iter()
                .map(|(at, nodes)| (at, nodes.into_values().collect()))
                .collect(),
        })
    }

    pub(crate) fn inside(&self, at: &Path) -> Option<&[Node]> {
        self.inside.get(at).map(Vec::as_slice)
    }

    pub(crate) fn node(&self, at: &Path) -> Option<&Node> {
        let parent = at.parent()?;
        let name = at.file_name()?.to_str()?;
        self.inside
            .get(parent)?
            .iter()
            .find(|node| node.name == name)
    }
}

fn claim(building: &mut HashMap<PathBuf, HashMap<String, Node>>, path: &Path, leaf: Node) {
    let Some(parent) = path.parent() else {
        return;
    };
    if !building.contains_key(parent) {
        ancestors(building, parent);
    }
    if leaf.directory {
        building.entry(path.to_path_buf()).or_default();
    }
    building
        .entry(parent.to_path_buf())
        .or_default()
        .insert(leaf.name.clone(), leaf);
}

fn ancestors(building: &mut HashMap<PathBuf, HashMap<String, Node>>, at: &Path) {
    let mut walked = PathBuf::new();
    for part in at.components() {
        let Component::Normal(name) = part else {
            continue;
        };
        let parent = walked.clone();
        walked.push(name);
        building.entry(walked.clone()).or_default();
        building
            .entry(parent)
            .or_default()
            .entry(name.to_string_lossy().into_owned())
            .or_insert_with(|| Node {
                name: name.to_string_lossy().into_owned(),
                directory: true,
                index: None,
                size: 0,
                modified: None,
                mode: None,
            });
    }
}

fn inside(name: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for part in name.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            part => path.push(part),
        }
    }
    (!path.as_os_str().is_empty()).then_some(path)
}
