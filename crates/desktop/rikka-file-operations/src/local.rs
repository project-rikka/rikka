use std::io::{BufWriter, ErrorKind, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rikka_fs::Staged;

use crate::entry::Entry;
use crate::error::{Action, Error, Result, check_name, name_of};
use crate::store::{Sink, Store};
use crate::trash;

const LINK_ATTEMPTS: u32 = 64;

#[derive(Clone, Copy, Debug, Default)]
pub struct Local;

impl Local {
    pub fn create_file(&self, at: &Path) -> Result<(), PathBuf> {
        check_name(name_of(at)?)?;
        std::fs::File::create_new(at)
            .map(drop)
            .map_err(|error| io(&error, at))
    }

    pub fn rename(&self, at: &Path, name: &str) -> Result<PathBuf, PathBuf> {
        check_name(name)?;
        let renamed = at
            .parent()
            .ok_or_else(|| Error::Denied {
                at: at.to_path_buf(),
            })?
            .join(name);
        if renamed == at {
            return Ok(renamed);
        }
        match rikka_fs::rename_new(at, &renamed) {
            Ok(()) => Ok(renamed),
            Err(error)
                if error.kind() == ErrorKind::AlreadyExists && is_same_entry(at, &renamed) =>
            {
                std::fs::rename(at, &renamed).map_err(|error| io(&error, at))?;
                Ok(renamed)
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                Err(Error::Exists { at: renamed })
            }
            Err(error) => Err(io(&error, at)),
        }
    }

    pub fn erase(&self, at: &Path) -> Result<(), PathBuf> {
        rikka_trash::erase(at).map_err(|error| trash::translated(error, Action::Anything))
    }

    pub fn set_metadata(
        &self,
        at: &Path,
        mode: u32,
        modified: Option<SystemTime>,
    ) -> Result<(), PathBuf> {
        let permissions = std::fs::Permissions::from_mode(mode & 0o7777);
        std::fs::set_permissions(at, permissions).map_err(|error| io(&error, at))?;
        if let Some(modified) = modified {
            rikka_fs::set_modified(at, modified).map_err(|error| io(&error, at))?;
        }
        Ok(())
    }

    #[must_use]
    pub fn is_writable(&self, folder: &Path) -> bool {
        rustix::fs::access(folder, rustix::fs::Access::WRITE_OK).is_ok()
    }
}

impl Store for Local {
    type Location = PathBuf;
    type Item = Entry;

    fn stat(&self, at: &PathBuf) -> Result<Entry, PathBuf> {
        Entry::read(at)
    }

    fn walk(&self, at: &PathBuf) -> Result<Vec<Entry>, PathBuf> {
        let listed = std::fs::read_dir(at).map_err(|error| io(&error, at))?;
        Ok(listed
            .flatten()
            .filter_map(|child| Entry::read(&child.path()).ok())
            .collect())
    }

    fn create_directory(&self, at: &PathBuf) -> Result<(), PathBuf> {
        check_name(name_of(at)?)?;
        std::fs::create_dir(at).map_err(|error| io(&error, at))
    }

    fn delete(&self, at: &PathBuf) -> Result<(), PathBuf> {
        let metadata = std::fs::symlink_metadata(at).map_err(|error| io(&error, at))?;
        let removed = if metadata.is_dir() {
            std::fs::remove_dir(at)
        } else {
            std::fs::remove_file(at)
        };
        removed.map_err(|error| io(&error, at))
    }

    fn trash(&self, at: &PathBuf) -> Result<PathBuf, PathBuf> {
        rikka_trash::trash(at).map_err(|error| trash::translated(error, Action::Trash))
    }

    fn restore(&self, at: &PathBuf) -> Result<PathBuf, PathBuf> {
        rikka_trash::restore(at).map_err(|error| trash::translated(error, Action::Restore))
    }

    fn read(&self, at: &PathBuf) -> Result<Box<dyn Read + Send>, PathBuf> {
        let file = std::fs::File::open(at).map_err(|error| io(&error, at))?;
        Ok(Box::new(std::io::BufReader::new(file)))
    }

    fn write(&self, at: &PathBuf) -> Result<Box<dyn Sink>, PathBuf> {
        let staged = rikka_fs::Replace::new(at)
            .unsynced()
            .stage()
            .map_err(|error| io(&error, at))?;
        Ok(Box::new(Draft(BufWriter::new(staged))))
    }

    fn link(&self, target: &Path, at: &PathBuf) -> Result<(), PathBuf> {
        check_name(name_of(at)?)?;
        let folder = at.parent().unwrap_or(Path::new("/"));
        let draft = (0..LINK_ATTEMPTS)
            .map(|attempt| folder.join(format!(".{}.link-{attempt}", std::process::id())))
            .find_map(|draft| match std::os::unix::fs::symlink(target, &draft) {
                Ok(()) => Some(Ok(draft)),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error)),
            })
            .unwrap_or_else(|| Err(ErrorKind::AlreadyExists.into()))
            .map_err(|error| io(&error, at))?;
        std::fs::rename(&draft, at).map_err(|error| {
            if let Err(left) = std::fs::remove_file(&draft) {
                tracing::warn!(%left, draft = %draft.display(), "failed to remove link draft");
            }
            io(&error, at)
        })
    }

    fn copy_metadata(&self, from: &Entry, to: &PathBuf) -> Result<(), PathBuf> {
        self.set_metadata(to, from.mode, from.modified)
    }

    fn can_rename_across(&self, from: &PathBuf, to: &PathBuf) -> bool {
        let source = match std::fs::symlink_metadata(from) {
            Ok(meta) if meta.is_dir() => from.as_path(),
            Ok(_) => from.parent().unwrap_or(from),
            Err(_) => return false,
        };
        let destination = to.parent().unwrap_or(to);
        match (device_of(source), device_of(destination)) {
            (Some(one), Some(other)) => one == other,
            _ => false,
        }
    }

    fn rename_across(&self, from: &PathBuf, to: &PathBuf) -> Result<(), PathBuf> {
        std::fs::rename(from, to).map_err(|error| io(&error, from))
    }
}

#[derive(Debug)]
struct Draft(BufWriter<Staged>);

impl Write for Draft {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.write(bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

impl Sink for Draft {
    fn commit(self: Box<Self>) -> std::io::Result<()> {
        self.0
            .into_inner()
            .map_err(std::io::IntoInnerError::into_error)?
            .commit()
    }
}

fn io(error: &std::io::Error, at: &Path) -> Error<PathBuf> {
    Error::from_io(error, &at.to_path_buf())
}

fn device_of(path: &Path) -> Option<u64> {
    std::fs::symlink_metadata(path).ok().map(|meta| meta.dev())
}

fn is_same_entry(one: &Path, other: &Path) -> bool {
    match (one.symlink_metadata(), other.symlink_metadata()) {
        (Ok(one), Ok(other)) => one.dev() == other.dev() && one.ino() == other.ino(),
        _ => false,
    }
}
