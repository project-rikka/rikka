use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::error::{Error, Result};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Profile {
    Persistent(PathBuf),
    #[default]
    Ephemeral,
}

impl Profile {
    #[must_use]
    pub fn is_ephemeral(&self) -> bool {
        matches!(self, Profile::Ephemeral)
    }

    pub(crate) fn prepare(&self) -> Result<ProfileFolder> {
        match self {
            Profile::Persistent(path) => {
                if !path.is_absolute() {
                    return Err(Error::RelativePath(path.clone()));
                }
                std::fs::create_dir_all(path).map_err(|source| Error::Profile {
                    path: path.clone(),
                    source,
                })?;
                Ok(ProfileFolder::Kept(path.clone()))
            }
            Profile::Ephemeral => tempfile::Builder::new()
                .prefix("rikka-gecko-")
                .tempdir()
                .map(ProfileFolder::Scratch)
                .map_err(|source| Error::Profile {
                    path: std::env::temp_dir(),
                    source,
                }),
        }
    }
}

#[derive(Debug)]
pub(crate) enum ProfileFolder {
    Kept(PathBuf),
    Scratch(TempDir),
}

impl ProfileFolder {
    pub(crate) fn path(&self) -> &Path {
        match self {
            ProfileFolder::Kept(path) => path,
            ProfileFolder::Scratch(folder) => folder.path(),
        }
    }
}
