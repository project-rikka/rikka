use std::path::{Component, Path, PathBuf};

use rikka_fs::resolve_symlinks;

use crate::error::{Error, Result};

#[derive(Debug)]
pub(crate) struct Workspace {
    root: PathBuf,
}

impl Workspace {
    pub(crate) fn open(root: &Path) -> Result<Workspace> {
        let root = root.canonicalize().map_err(|error| Error::Io {
            path: root.display().to_string(),
            reason: error.to_string(),
        })?;
        Ok(Workspace { root })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn resolve(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        match path.is_absolute() {
            true => normalize(path),
            false => normalize(&self.root.join(path)),
        }
    }

    pub(crate) fn resolve_inside(&self, path: &str) -> Result<PathBuf> {
        let resolved = self.resolve(path);
        let target = resolve_symlinks(&resolved)
            .map_or_else(|_| resolved.clone(), |target| normalize(&target));
        match canonical_prefix(&target).starts_with(&self.root) {
            true => Ok(resolved),
            false => Err(Error::OutsideWorkspace(self.relative_path(&resolved))),
        }
    }

    pub(crate) fn relative_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .display()
            .to_string()
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            part => normalized.push(part),
        }
    }
    normalized
}

fn canonical_prefix(path: &Path) -> PathBuf {
    let mut ancestor = path.to_path_buf();
    let mut rest = Vec::new();

    loop {
        if let Ok(canonical_prefix) = ancestor.canonicalize() {
            return rest
                .iter()
                .rev()
                .fold(canonical_prefix, |canonical_prefix, leaf| {
                    canonical_prefix.join(leaf)
                });
        }

        let (Some(parent), Some(leaf)) = (ancestor.parent(), ancestor.file_name()) else {
            return path.to_path_buf();
        };
        rest.push(leaf.to_owned());
        ancestor = parent.to_path_buf();
    }
}
