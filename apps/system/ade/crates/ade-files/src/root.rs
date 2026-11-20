use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_file_operations::{Local, Store, Task};

use crate::error::{Error, Result};
use crate::job::{Job, JobKind};
use crate::name;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    File,
    Folder,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moved {
    pub from: PathBuf,
    pub to: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Created {
    pub path: PathBuf,
    pub kind: Kind,
}

#[derive(Clone, Debug)]
pub struct Root {
    root: PathBuf,
}

impl Root {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Root {
        Root { root: root.into() }
    }

    fn ensure_inside(&self, path: &Path) -> Result<()> {
        match path.starts_with(&self.root) && path != self.root {
            true => Ok(()),
            false => Err(Error::Outside(path.to_owned())),
        }
    }

    fn folder(&self, folder: &Path) -> Result<()> {
        match folder.starts_with(&self.root) {
            true => Ok(()),
            false => Err(Error::Outside(folder.to_owned())),
        }
    }

    pub fn create(&self, folder: &Path, typed: &str, kind: Kind) -> Result<Created> {
        self.folder(folder)?;
        let named = name::parse(typed)?;
        let path = folder.join(&named.relative);
        if path.symlink_metadata().is_ok() {
            return Err(Error::Exists(path));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                rikka_file_operations::Error::from_io(&error, &parent.to_path_buf())
            })?;
        }
        let kind = match (kind, named.folder) {
            (Kind::Folder, _) | (Kind::File, true) => {
                Local.create_directory(&path)?;
                Kind::Folder
            }
            (Kind::File, false) => {
                Local.create_file(&path)?;
                Kind::File
            }
        };
        Ok(Created { path, kind })
    }

    pub fn rename(&self, path: &Path, typed: &str) -> Result<PathBuf> {
        self.ensure_inside(path)?;
        let name = name::parse_component(typed)?;
        Ok(Local.rename(path, &name)?)
    }

    pub fn move_into(&self, paths: &[PathBuf], folder: &Path) -> Result<Job> {
        self.folder(folder)?;
        self.all_inside(paths)?;
        Ok(started(
            Task::Move {
                sources: paths.to_vec(),
                into: folder.to_owned(),
            },
            JobKind::Move,
            paths,
        ))
    }

    pub fn copy_into(&self, paths: &[PathBuf], folder: &Path) -> Result<Job> {
        self.folder(folder)?;
        Ok(started(
            Task::Copy {
                sources: paths.to_vec(),
                into: folder.to_owned(),
            },
            JobKind::Copy,
            paths,
        ))
    }

    pub fn trash(&self, paths: &[PathBuf]) -> Result<Job> {
        self.all_inside(paths)?;
        Ok(started(
            Task::Trash {
                items: paths.to_vec(),
            },
            JobKind::Trash,
            paths,
        ))
    }

    pub fn delete(&self, paths: &[PathBuf]) -> Result<Job> {
        self.all_inside(paths)?;
        Ok(started(
            Task::Delete {
                items: paths.to_vec(),
            },
            JobKind::Delete,
            paths,
        ))
    }

    fn all_inside(&self, paths: &[PathBuf]) -> Result<()> {
        paths.iter().try_for_each(|path| self.ensure_inside(path))
    }
}

fn started(task: Task<PathBuf>, kind: JobKind, asked: &[PathBuf]) -> Job {
    Job::new(
        rikka_file_operations::run(Arc::new(Local), task),
        kind,
        asked,
    )
}
