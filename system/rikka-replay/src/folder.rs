use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use rikka_dirs::{UserDir, UserDirs};

use crate::error::{Error, Result};

const FOLDER: &str = "Replays";
const STEM: &str = "Replay";
const EXTENSION: &str = "mp4";
const DRAFTS: &str = "replay-drafts";
const DRAFT: &str = "draft-";
const MAX_NAME_SUFFIX: u32 = 100;

pub(crate) fn output_folder(named: Option<&Path>) -> Result<PathBuf> {
    let folder = match named {
        Some(named) => named.to_path_buf(),
        None => UserDirs::load()
            .path(UserDir::Videos)
            .or_else(|| Some(rikka_dirs::home()?.to_path_buf()))
            .ok_or(Error::NoFolder)?
            .join(FOLDER),
    };
    std::fs::create_dir_all(&folder).map_err(|error| Error::CreateFolder(folder.clone(), error))?;
    Ok(folder)
}

pub(crate) fn write_new_file<T>(
    folder: &Path,
    write: impl FnOnce(&Path) -> Result<T>,
) -> Result<(PathBuf, T)> {
    let file = create_unique_file(folder)?;
    match write(&file) {
        Ok(written) => Ok((file, written)),
        Err(error) => {
            remove_file(&file);
            Err(error)
        }
    }
}

fn create_unique_file(folder: &Path) -> Result<PathBuf> {
    let when = jiff::Zoned::now().strftime("%Y-%m-%d %H-%M-%S");
    let stem = format!("{STEM} {when}");
    let names = std::iter::once(format!("{stem}.{EXTENSION}"))
        .chain((2..MAX_NAME_SUFFIX).map(|count| format!("{stem} ({count}).{EXTENSION}")));
    for name in names {
        let file = folder.join(name);
        match std::fs::File::create_new(&file) {
            Ok(_) => return Ok(file),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(Error::Write(file, error)),
        }
    }
    Err(Error::NameCollision(folder.to_path_buf()))
}

#[derive(Debug)]
pub(crate) struct Drafts {
    folder: PathBuf,
}

impl Drafts {
    pub(crate) fn open() -> Result<Drafts> {
        let folder = rikka_seam::net::socket_path()
            .as_deref()
            .and_then(Path::parent)
            .ok_or(Error::NoFolder)?
            .join(DRAFTS);
        std::fs::create_dir_all(&folder)
            .map_err(|error| Error::CreateFolder(folder.clone(), error))?;
        let stale = std::fs::read_dir(&folder)
            .map_err(|error| Error::CreateFolder(folder.clone(), error))?;
        for entry in stale.flatten() {
            remove_file(&entry.path());
        }
        Ok(Drafts { folder })
    }

    #[must_use]
    pub(crate) fn file(&self, serial: u64) -> PathBuf {
        self.folder.join(format!("{DRAFT}{serial}.{EXTENSION}"))
    }

    #[must_use]
    pub(crate) fn contains_draft(&self, path: &Path) -> bool {
        path.parent() == Some(self.folder.as_path())
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(DRAFT) && name.ends_with(EXTENSION))
    }

    #[must_use]
    pub(crate) fn available_bytes(&self) -> Option<u64> {
        match rustix::fs::statvfs(&self.folder) {
            Ok(stats) => Some(stats.f_bavail.saturating_mul(stats.f_frsize)),
            Err(error) => {
                tracing::warn!(%error, folder = %self.folder.display(), "cannot query free space");
                None
            }
        }
    }
}

pub(crate) fn remove_file(path: &Path) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => tracing::warn!(%error, path = %path.display(), "cannot remove file"),
    }
}
