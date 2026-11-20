use std::io;
use std::path::{Path, PathBuf};

use rikka_proton_api::offload;
use rikka_proton_names::numbered_name;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt as _;

const PARTIAL_PREFIX: &str = ".rikka-drive-";
const PARTIAL_SUFFIX: &str = ".part";
const PARTIAL_TOKEN_LEN: usize = 8;
const FALLBACK_NAME: &str = "file";
const MAX_NAME_NUMBER: u32 = 10_000;

pub(crate) struct PartialFile {
    file: File,
    path: PathBuf,
    folder: PathBuf,
    placed: bool,
}

impl PartialFile {
    pub(crate) async fn create(folder: &Path) -> io::Result<PartialFile> {
        let token = rikka_proton_pgp::random_hex(PARTIAL_TOKEN_LEN);
        let path = folder.join(format!("{PARTIAL_PREFIX}{token}{PARTIAL_SUFFIX}"));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await?;

        Ok(PartialFile {
            file,
            path,
            folder: folder.to_owned(),
            placed: false,
        })
    }

    pub(crate) async fn write(&mut self, block: &[u8]) -> io::Result<()> {
        self.file.write_all(block).await
    }

    pub(crate) async fn place(mut self, name: &str) -> io::Result<PathBuf> {
        self.file.flush().await?;
        self.file.sync_all().await?;

        let (from, folder, name) = (self.path.clone(), self.folder.clone(), local_name(name));
        let placed = offload(move || move_to_free_name(&from, &folder, &name)).await?;
        self.placed = true;
        Ok(placed)
    }
}

impl Drop for PartialFile {
    fn drop(&mut self) {
        if !self.placed {
            std::fs::remove_file(&self.path).ok();
        }
    }
}

fn local_name(name: &str) -> String {
    Path::new(name)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| FALLBACK_NAME.to_owned())
}

fn move_to_free_name(from: &Path, folder: &Path, name: &str) -> io::Result<PathBuf> {
    let numbered_names = (1..MAX_NAME_NUMBER).map(|number| numbered_name(name, number));

    for candidate in std::iter::once(name.to_owned()).chain(numbered_names) {
        let target = folder.join(candidate);
        match rikka_fs::rename_new(from, &target) {
            Ok(()) => return Ok(target),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::from(io::ErrorKind::AlreadyExists))
}
