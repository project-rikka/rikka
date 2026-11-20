use std::fs::{File, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use rikka_secrets::{Entry, Secrets, schema};
use sha2::{Digest, Sha256};

use crate::credentials::Credentials;

const LOCKS_DIR: &str = "mcp-locks";
const LOCK_MODE: u32 = 0o600;
const LOCKS_MODE: u32 = 0o700;
const KEY_BYTES: usize = 16;
const SERVER: &str = "server";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vault {
    locks: PathBuf,
}

impl Vault {
    #[must_use]
    pub fn from_runtime_dir() -> Option<Vault> {
        let paths = rikka_dirs::Paths::resolve().ok()?;
        Some(Vault {
            locks: paths.runtime_dir()?.join(LOCKS_DIR),
        })
    }

    pub(crate) fn contains(&self, url: &str) -> bool {
        let found = rikka_secrets::blocking::Secrets::shared()
            .and_then(|secrets| secrets.contains(&entry(url)));
        match found {
            Ok(found) => found,
            Err(rikka_secrets::Error::Locked) => false,
            Err(error) => {
                tracing::warn!(%error, url, "failed to look up stored sign-in");
                false
            }
        }
    }

    pub(crate) async fn remove(&self, url: &str) -> rikka_secrets::Result<()> {
        Secrets::unlocked()
            .await?
            .remove(&entry(url))
            .await
            .map(|_| ())
    }

    pub(crate) fn entry(&self, url: &str) -> CredentialEntry {
        CredentialEntry {
            url: normalized(url).to_owned(),
            lock: self.locks.join(format!("{}.lock", key(url))),
        }
    }
}

fn normalized(url: &str) -> &str {
    url.trim().trim_end_matches('/')
}

fn entry(url: &str) -> Entry {
    Entry::new(schema::MCP).attribute(SERVER, normalized(url))
}

fn key(url: &str) -> String {
    let digest = Sha256::digest(normalized(url).as_bytes());
    rikka_types::hex::encode(&digest[..KEY_BYTES])
}

#[derive(Clone, Debug)]
pub(crate) struct CredentialEntry {
    url: String,
    lock: PathBuf,
}

#[derive(Debug)]
pub(crate) struct RenewalLock {
    _file: File,
}

impl CredentialEntry {
    pub(crate) async fn load(&self) -> rikka_secrets::Result<Option<Credentials>> {
        let secrets = Secrets::unlocked().await?;
        match secrets.lookup_json(&entry(&self.url)).await {
            Err(rikka_secrets::Error::Encoding(error)) => {
                tracing::warn!(%error, url = %self.url, "ignoring unreadable credentials");
                Ok(None)
            }
            loaded => loaded,
        }
    }

    pub(crate) async fn save(&self, credentials: &Credentials) -> rikka_secrets::Result<()> {
        Secrets::unlocked()
            .await?
            .store_json(
                &entry(&self.url),
                &format!("MCP sign-in for {}", self.url),
                credentials,
            )
            .await
    }

    pub(crate) async fn lock_renewal(&self) -> std::io::Result<RenewalLock> {
        let lock = self.lock.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(dir) = lock.parent() {
                create_private_dir(dir)?;
            }
            let file = OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .mode(LOCK_MODE)
                .open(&lock)?;
            file.lock()?;
            Ok(RenewalLock { _file: file })
        })
        .await
        .map_err(std::io::Error::other)?
    }
}

fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(LOCKS_MODE)
        .create(dir)
}
