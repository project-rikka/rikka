use std::fs::{File, Metadata};
use std::io::{self, Read as _};
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use rikka_proton_drive_sync_state::TEMPORARY_PREFIX;
use rikka_proton_names::numbered_local_name;
use rikka_proton_pgp::Sha1;

const RECYCLE: &str = ".~rikka-sync-recycle";
const READ_CHUNK: usize = 1024 * 1024;
const MAX_NAME_NUMBER: u32 = 10_000;

pub(crate) fn modified(metadata: &Metadata) -> i64 {
    metadata
        .mtime()
        .saturating_mul(1_000_000_000)
        .saturating_add(metadata.mtime_nsec())
}

pub(crate) fn file_sha1(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha1::new();
    let mut chunk = vec![0u8; READ_CHUNK];
    loop {
        let read = file.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        digest.update(&chunk[..read]);
    }
    Ok(rikka_types::hex::encode(digest.finalize()))
}

pub(crate) fn temp_file_path(folder: &Path) -> PathBuf {
    let token = rikka_proton_pgp::random_hex(8);
    folder.join(format!("{TEMPORARY_PREFIX}{token}.part"))
}

pub(crate) fn exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

pub(crate) fn available_local_name(folder: &Path, name: &str) -> io::Result<String> {
    if !exists(&folder.join(name)) {
        return Ok(name.to_owned());
    }
    (1..MAX_NAME_NUMBER)
        .map(|number| numbered_local_name(name, number))
        .find(|candidate| !exists(&folder.join(candidate)))
        .ok_or_else(|| io::Error::from(io::ErrorKind::AlreadyExists))
}

pub(crate) fn move_to_available_name(from: &Path, folder: &Path, name: &str) -> io::Result<String> {
    let mut candidate = name.to_owned();
    for _ in 0..MAX_NAME_NUMBER {
        let target = folder.join(&candidate);
        match rikka_fs::rename_new(from, &target) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                candidate = available_local_name(folder, name)?;
            }
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::from(io::ErrorKind::AlreadyExists))
}

pub(crate) fn recycle(path: &Path, root: &Path) -> io::Result<PathBuf> {
    match rikka_trash::trash(path) {
        Ok(trashed) => Ok(trashed),
        Err(error) => {
            tracing::debug!(path = %path.display(), %error, "trash failed, using recycle folder");
            let folder = root.join(RECYCLE);
            std::fs::create_dir_all(&folder)?;
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "item".to_owned());
            let final_name = move_to_available_name(path, &folder, &name)?;
            Ok(folder.join(final_name))
        }
    }
}

pub(crate) fn sync_folder(folder: &Path) -> io::Result<()> {
    File::open(folder)?.sync_all()
}

pub(crate) struct TempFile {
    path: PathBuf,
    persisted: bool,
}

impl TempFile {
    pub(crate) fn new(path: PathBuf) -> TempFile {
        TempFile {
            path,
            persisted: false,
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn keep(mut self) {
        self.persisted = true;
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.persisted {
            std::fs::remove_file(&self.path).ok();
        }
    }
}
