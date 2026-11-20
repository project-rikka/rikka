use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

use crate::error::Result;
use crate::{ops, runtime};

const PRUNER: &str = "rikka-net-cache";

#[derive(Clone, Debug)]
pub struct DiskCache {
    folder: PathBuf,
}

impl DiskCache {
    pub fn open(folder: impl Into<PathBuf>, max_age: Duration) -> Result<DiskCache> {
        let folder = folder.into();
        std::fs::create_dir_all(&folder)?;
        let pruned = folder.clone();
        std::thread::Builder::new()
            .name(PRUNER.to_owned())
            .spawn(move || remove_stale(&pruned, max_age))?;
        Ok(DiskCache { folder })
    }

    #[must_use]
    pub fn path_for(&self, key: &str, extension: &str) -> PathBuf {
        let name = rikka_types::hex::encode(&Sha256::digest(key.as_bytes())[..12]);
        match extension.is_empty() {
            true => self.folder.join(name),
            false => self.folder.join(format!("{name}.{extension}")),
        }
    }

    #[must_use]
    pub fn read(&self, key: &str) -> Option<Vec<u8>> {
        let mut file = File::open(self.path_for(key, "")).ok()?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).ok()?;
        if let Err(error) = file.set_modified(SystemTime::now()) {
            tracing::debug!(%error, key, "failed to mark cache entry used");
        }
        Some(bytes)
    }

    pub fn write(&self, key: &str, bytes: &[u8]) -> Result<()> {
        rikka_fs::Replace::new(self.path_for(key, ""))
            .private()
            .write(bytes)?;
        Ok(())
    }

    pub fn get_or_fetch(&self, url: &str) -> Result<Arc<[u8]>> {
        if let Some(cached) = self.read(url) {
            return Ok(cached.into());
        }
        let fetched = runtime::block_on_local(ops::shared().get_bytes(url))?;
        if let Err(error) = self.write(url, &fetched) {
            tracing::debug!(%error, url, "failed to cache download");
        }
        Ok(Arc::from(&fetched[..]))
    }
}

fn remove_stale(folder: &Path, max_age: Duration) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .is_ok_and(|modified| modified.elapsed().is_ok_and(|age| age > max_age));
        if stale && let Err(error) = std::fs::remove_file(entry.path()) {
            tracing::debug!(%error, path = %entry.path().display(), "failed to remove stale cache file");
        }
    }
}
