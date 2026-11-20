use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Store {
    root: PathBuf,
    top: Option<PathBuf>,
}

impl Store {
    pub(crate) fn files(&self) -> PathBuf {
        self.root.join("files")
    }

    pub(crate) fn info(&self) -> PathBuf {
        self.root.join("info")
    }

    pub(crate) fn ensure(&self) -> Result<()> {
        for dir in [self.files(), self.info()] {
            std::fs::create_dir_all(&dir).map_err(Error::io(&dir))?;
        }
        Ok(())
    }

    pub(crate) fn record_path(&self, original: &Path) -> PathBuf {
        match &self.top {
            None => original.to_path_buf(),
            Some(top) => original.strip_prefix(top).unwrap_or(original).to_path_buf(),
        }
    }

    pub(crate) fn resolve_path(&self, recorded: &Path) -> PathBuf {
        match &self.top {
            None => recorded.to_path_buf(),
            Some(top) => top.join(recorded),
        }
    }

    pub(crate) fn home() -> Option<Self> {
        Some(Self {
            root: rikka_dirs::data_home()?.join("Trash"),
            top: None,
        })
    }

    pub(crate) fn for_path(path: &Path) -> Result<Self> {
        let folder = path
            .parent()
            .ok_or_else(|| Error::Unsupported(path.to_owned()))?;
        let home = Self::home();
        if let Some(home) = &home
            && same_filesystem(folder, &home.root)
        {
            return Ok(home.clone());
        }

        let top = top_directory(folder).ok_or_else(|| Error::Unsupported(path.to_owned()))?;
        let uid = current_uid();

        let administrated = top.join(".Trash");
        if is_sticky_directory(&administrated) {
            return Ok(Self {
                root: administrated.join(uid.to_string()),
                top: Some(top),
            });
        }
        Ok(Self {
            root: top.join(format!(".Trash-{uid}")),
            top: Some(top),
        })
    }

    pub(crate) fn all() -> Vec<Self> {
        let mut stores: Vec<Self> = Self::home().into_iter().collect();
        let uid = current_uid();
        stores.extend(
            rikka_mounts::top_directories()
                .iter()
                .flat_map(|top| Self::at_top(top, uid)),
        );
        stores.retain(|store| store.root.is_dir());
        stores
    }

    pub(crate) fn containing(path: &Path) -> Option<Self> {
        Self::all()
            .into_iter()
            .find(|store| path.starts_with(store.files()))
            .or_else(|| Self::enclosing(path))
    }

    fn enclosing(path: &Path) -> Option<Self> {
        let uid = current_uid();
        path.ancestors()
            .skip(1)
            .flat_map(|top| Self::at_top(top, uid))
            .find(|store| path.starts_with(store.files()))
    }

    fn at_top(top: &Path, uid: u32) -> Vec<Self> {
        let administrated = top.join(".Trash");
        [
            is_sticky_directory(&administrated).then(|| administrated.join(uid.to_string())),
            Some(top.join(format!(".Trash-{uid}"))),
        ]
        .into_iter()
        .flatten()
        .filter(|root| root.is_dir())
        .map(|root| Self {
            root,
            top: Some(top.to_path_buf()),
        })
        .collect()
    }
}

fn current_uid() -> u32 {
    rustix::process::getuid().as_raw()
}

fn same_filesystem(one: &Path, other: &Path) -> bool {
    match (device_of(one), device_of(other)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

fn device_of(path: &Path) -> Option<u64> {
    let mut candidate = path;
    loop {
        if let Ok(metadata) = std::fs::symlink_metadata(candidate) {
            return Some(metadata.dev());
        }
        candidate = candidate.parent()?;
    }
}

fn top_directory(folder: &Path) -> Option<PathBuf> {
    let device = device_of(folder)?;
    let mut top = folder.to_path_buf();
    while let Some(parent) = top.parent() {
        if device_of(parent) != Some(device) {
            break;
        }
        top = parent.to_path_buf();
    }
    Some(top)
}

fn is_sticky_directory(path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    metadata.is_dir() && metadata.mode() & 0o1000 != 0
}
