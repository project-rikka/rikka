use std::path::{Path, PathBuf};

use rikka_dirs::{UserDir, UserDirs};
use rustix::fs::{CWD, RenameFlags};
use tokio::io::AsyncWriteExt;

const DOWNLOADS: &str = "Downloads";
const ATTEMPTS: u32 = 100;
const CHUNK: usize = 1 << 20;

#[derive(Debug, thiserror::Error)]
pub(crate) enum UploadError {
    #[error("no folder to keep received files in")]
    NoFolder,
    #[error("the disk is full")]
    NoSpace,
    #[error("the upload broke off: {0}")]
    Stream(quinn::ReadError),
    #[error("the upload carried {got} bytes instead of {declared}")]
    Size { declared: u64, got: u64 },
    #[error("cannot write {path}: {error}", path = .path.display())]
    Write {
        path: PathBuf,
        error: std::io::Error,
    },
    #[error("too many files are named {0}")]
    Taken(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Received {
    pub(crate) name: String,
    pub(crate) folder: PathBuf,
}

fn folder() -> Option<PathBuf> {
    let dirs = UserDirs::load();
    dirs.path(UserDir::Downloads)
        .or_else(|| dirs.home().map(|home| home.join(DOWNLOADS)))
}

pub(crate) async fn receive(
    name: &str,
    declared: u64,
    head: &[u8],
    stream: &mut quinn::RecvStream,
) -> Result<Received, UploadError> {
    let folder = folder().ok_or(UploadError::NoFolder)?;
    tokio::fs::create_dir_all(&folder)
        .await
        .map_err(|error| written(&folder, error))?;
    let nonce = getrandom::u64().map_err(|error| written(&folder, std::io::Error::other(error)))?;
    let part = folder.join(format!(".{name}.{nonce:016x}.part"));
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&part)
        .await
        .map_err(|error| written(&part, error))?;
    let copied = async {
        let mut got = head.len() as u64;
        if got > declared {
            return Err(UploadError::Size { declared, got });
        }
        file.write_all(head)
            .await
            .map_err(|error| written(&part, error))?;
        while let Some(chunk) = stream
            .read_chunk(CHUNK, true)
            .await
            .map_err(UploadError::Stream)?
        {
            got += chunk.bytes.len() as u64;
            if got > declared {
                return Err(UploadError::Size { declared, got });
            }
            file.write_all(&chunk.bytes)
                .await
                .map_err(|error| written(&part, error))?;
        }
        if got != declared {
            return Err(UploadError::Size { declared, got });
        }
        file.sync_all()
            .await
            .map_err(|error| written(&part, error))?;
        claim(&part, &folder, name)
    }
    .await;
    if copied.is_err()
        && let Err(error) = tokio::fs::remove_file(&part).await
    {
        tracing::warn!("cannot remove {}: {error}", part.display());
    }
    copied.map(|name| Received { name, folder })
}

fn claim(part: &Path, folder: &Path, name: &str) -> Result<String, UploadError> {
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => (stem, Some(extension)),
        _ => (name, None),
    };
    for attempt in 1..=ATTEMPTS {
        let candidate = match (attempt, extension) {
            (1, _) => name.to_owned(),
            (_, Some(extension)) => format!("{stem} ({attempt}).{extension}"),
            (_, None) => format!("{stem} ({attempt})"),
        };
        let target = folder.join(&candidate);
        match rustix::fs::renameat_with(CWD, part, CWD, &target, RenameFlags::NOREPLACE) {
            Ok(()) => return Ok(candidate),
            Err(rustix::io::Errno::EXIST) => continue,
            Err(error) => return Err(written(&target, error.into())),
        }
    }
    Err(UploadError::Taken(name.to_owned()))
}

fn written(path: &Path, error: std::io::Error) -> UploadError {
    match error.raw_os_error() {
        Some(code) if code == rustix::io::Errno::NOSPC.raw_os_error() => UploadError::NoSpace,
        _ => UploadError::Write {
            path: path.to_path_buf(),
            error,
        },
    }
}
