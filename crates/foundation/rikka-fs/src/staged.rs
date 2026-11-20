use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, RandomState};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::fd::{AsFd, BorrowedFd};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as _;
#[cfg(windows)]
use std::os::windows::io::{AsHandle, BorrowedHandle};
use std::path::{Path, PathBuf};

use crate::permissions;
use crate::rename::{rename_new, rename_over};
use crate::replace::{Durability, Mode};
use crate::symlinks::resolve_symlinks;

#[cfg(unix)]
const STAGING_MODE: u32 = 0o600;
const STAGING_SUFFIX: &str = ".new";
const MAX_NAME_ATTEMPTS: usize = 64;

#[derive(Debug)]
pub struct Staged {
    file: File,
    staging: PathBuf,
    target: PathBuf,
    mode: Mode,
    durability: Durability,
    committed: bool,
}

impl Staged {
    pub(crate) fn create(
        path: &Path,
        mode: Mode,
        durability: Durability,
        keep_extension: bool,
    ) -> io::Result<Staged> {
        let target = resolve_symlinks(path)?;
        let folder = folder_of(&target);
        fs::create_dir_all(&folder)?;
        let name = target
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
        let extension = target.extension().filter(|_| keep_extension);

        let names = RandomState::new();
        for attempt in 0..MAX_NAME_ATTEMPTS {
            let mut staging_name = OsString::from(".");
            staging_name.push(name);
            staging_name.push(format!(".{:016x}", names.hash_one(attempt)));
            staging_name.push(STAGING_SUFFIX);
            if let Some(extension) = extension {
                staging_name.push(".");
                staging_name.push(extension);
            }
            let staging = folder.join(staging_name);
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            options.mode(STAGING_MODE);
            match options.open(&staging) {
                Ok(file) => {
                    return Ok(Staged {
                        file,
                        staging,
                        target,
                        mode,
                        durability,
                        committed: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("no free staging name beside {}", target.display()),
        ))
    }

    pub fn target(&self) -> &Path {
        &self.target
    }

    pub fn path(&self) -> &Path {
        &self.staging
    }

    pub fn commit(mut self) -> io::Result<()> {
        self.settle(&self.target)?;
        rename_over(&self.staging, &self.target)?;
        self.committed = true;
        #[cfg(unix)]
        self.durability.sync_folder(&folder_of(&self.target))?;
        Ok(())
    }

    pub fn commit_unused(
        mut self,
        candidates: impl IntoIterator<Item = PathBuf>,
    ) -> io::Result<PathBuf> {
        for candidate in candidates {
            self.settle(&candidate)?;
            match rename_new(&self.staging, &candidate) {
                Ok(()) => {
                    self.committed = true;
                    #[cfg(unix)]
                    self.durability.sync_folder(&folder_of(&candidate))?;
                    return Ok(candidate);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("every name for {} is taken", self.target.display()),
        ))
    }

    fn settle(&self, target: &Path) -> io::Result<()> {
        (&self.file).flush()?;
        permissions::apply(&self.staging, self.mode, target)?;
        match self.durability {
            Durability::Synced => self.file.sync_all(),
            Durability::Unsynced => Ok(()),
        }
    }
}

impl Write for Staged {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(unix)]
impl AsFd for Staged {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }
}

#[cfg(windows)]
impl AsHandle for Staged {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.file.as_handle()
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.staging);
        }
    }
}

fn folder_of(target: &Path) -> PathBuf {
    match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

#[cfg(unix)]
impl Durability {
    fn sync_folder(self, folder: &Path) -> io::Result<()> {
        if self == Durability::Unsynced {
            return Ok(());
        }
        match File::open(folder)?.sync_all() {
            Err(error) if error.kind() == io::ErrorKind::InvalidInput => Ok(()),
            synced => synced,
        }
    }
}
