use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_dirs::Paths;
use rikka_proton_api::private_file;
use rikka_proton_cache::Cache;
use rikka_proton_client::{SealedFile, SealedSessionStore};
use rikka_proton_key_transparency::LogLists;
use rikka_proton_pgp::SymmetricKey;
use rikka_secrets::blocking::Secrets;
use rikka_secrets::{Entry, schema};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const HOME: &str = "proton";
const SESSION: &str = "session.sealed";
const SECRET: &str = "account.sealed";
const CACHE: &str = "cache";
const LOG_LISTS: &str = "certificate-transparency";
const SESSION_SEALED_FOR: &[u8] = b"rikka.proton.session";
const SEALED_FOR: &[u8] = b"rikka.proton.account";
const KEY_STORE: &str = "store";
const KEY_LABEL: &str = "Proton saved sign-in key";

#[derive(Clone, Serialize, Deserialize)]
pub struct SavedAccount {
    pub username: String,
    pub passphrase: Option<String>,
}

impl std::fmt::Debug for SavedAccount {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SavedAccount")
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

pub struct AccountStore {
    inner: Arc<Inner>,
}

struct Inner {
    sessions: SealedFile,
    account: SealedFile,
    cache: Cache,
    log_lists: LogLists,
}

impl AccountStore {
    pub fn open() -> Result<AccountStore> {
        let paths = Paths::resolve()?;
        AccountStore::open_at(
            paths.state_dir().join(HOME),
            paths.cache_dir().join(LOG_LISTS),
        )
    }

    pub fn open_in(dir: impl Into<PathBuf>) -> Result<AccountStore> {
        let dir = dir.into();
        let log_list_dir = dir.join(LOG_LISTS);
        AccountStore::open_at(dir, log_list_dir)
    }

    fn open_at(dir: PathBuf, log_list_dir: PathBuf) -> Result<AccountStore> {
        private_file::create_private_dir(&dir).map_err(io_error_at(&dir))?;
        let key = key(&dir)?;
        let cache = Cache::open(dir.join(CACHE), key.clone())?;
        Ok(AccountStore {
            inner: Arc::new(Inner {
                sessions: SealedFile::new(dir.join(SESSION), key.clone(), SESSION_SEALED_FOR),
                account: SealedFile::new(dir.join(SECRET), key, SEALED_FOR),
                cache,
                log_lists: LogLists::builder().cache_dir(log_list_dir).build(),
            }),
        })
    }

    #[must_use]
    pub fn sessions(&self) -> SealedSessionStore {
        SealedSessionStore::new(self.inner.sessions.clone())
    }

    #[must_use]
    pub fn cache(&self) -> Cache {
        self.inner.cache.current_era()
    }

    #[must_use]
    pub fn log_lists(&self) -> LogLists {
        self.inner.log_lists.clone()
    }

    #[must_use]
    pub fn saved_account(&self) -> Option<SavedAccount> {
        match self.inner.account.read() {
            Ok(saved_account) => saved_account,
            Err(error) => {
                tracing::warn!(%error, "failed to open saved account");
                None
            }
        }
    }

    pub fn save_account(&self, saved_account: &SavedAccount) -> Result<()> {
        let sealed = &self.inner.account;
        sealed
            .write(saved_account)
            .map_err(io_error_at(sealed.path()))
    }

    pub fn forget(&self) {
        drop(self.inner.account.remove());
        drop(self.inner.sessions.remove());
        drop(self.inner.cache.retire());
    }
}

impl Clone for AccountStore {
    fn clone(&self) -> AccountStore {
        AccountStore {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl std::fmt::Debug for AccountStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccountStore")
            .field("account", &self.inner.account)
            .finish_non_exhaustive()
    }
}

fn key(dir: &Path) -> Result<SymmetricKey> {
    let secrets = Secrets::shared()?;
    let entry = Entry::new(schema::PROTON).attribute(KEY_STORE, dir.to_string_lossy());
    if let Some(stored) = secrets.lookup(&entry)? {
        match SymmetricKey::from_bytes(stored.expose()) {
            Ok(key) => return Ok(key),
            Err(error) => tracing::warn!(%error, "replacing an unreadable Proton store key"),
        }
    }
    let key = SymmetricKey::generate();
    secrets.store(&entry, KEY_LABEL, key.as_bytes())?;
    Ok(key)
}

fn io_error_at(path: &Path) -> impl FnOnce(std::io::Error) -> Error {
    let path = path.to_path_buf();
    move |source| Error::Io { path, source }
}
