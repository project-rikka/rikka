use rikka_sync::{Mutex, MutexGuard};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_proton_account::{AccountKeys, UserId};
use rikka_proton_api::{MemorySessionStore, offload, private_file};
use rikka_proton_cache::Cache;
use rikka_proton_pgp::{Hash, SymmetricKey, hmac, sha256};
use serde::{Deserialize, Serialize};

use crate::client::Proton;
use crate::error::{Error, Result};
use crate::sealed::{SealedFile, SealedSessionStore};
use crate::session::Session;

const ACCOUNT_LIST_FILE: &str = "accounts.sealed";
const SESSION_FILE: &str = "session.sealed";
const CACHE_DIR: &str = "cache";
const ACCOUNT_LIST_PURPOSE: &[u8] = b"rikka-proton-client/accounts";
const SESSION_PURPOSE: &[u8] = b"rikka-proton-client/session";
const ACCOUNT_KEY_CONTEXT: &str = "rikka-proton-client/account/";
const ACCOUNT_DIR_HASH_LEN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccountState {
    Ready,
    Disabled,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountInfo {
    user: UserId,
    name: Option<String>,
    email: Option<String>,
    state: AccountState,
    primary: bool,
}

impl AccountInfo {
    #[must_use]
    pub fn user(&self) -> &UserId {
        &self.user
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    #[must_use]
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    #[must_use]
    pub fn state(&self) -> AccountState {
        self.state
    }

    #[must_use]
    pub fn is_primary(&self) -> bool {
        self.primary
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AccountList {
    primary: Option<UserId>,
    accounts: Vec<StoredAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAccount {
    user: UserId,
    name: Option<String>,
    email: Option<String>,
    state: AccountState,
}

impl AccountList {
    fn get(&self, user: &UserId) -> Option<&StoredAccount> {
        self.accounts.iter().find(|account| &account.user == user)
    }

    fn is_ready(&self, user: &UserId) -> bool {
        self.get(user)
            .is_some_and(|account| account.state == AccountState::Ready)
    }

    fn ensure_primary(&mut self) {
        if self
            .primary
            .as_ref()
            .is_some_and(|user| self.is_ready(user))
        {
            return;
        }
        self.primary = self
            .accounts
            .iter()
            .find(|account| account.state == AccountState::Ready)
            .map(|account| account.user.clone());
    }
}

#[derive(Clone)]
pub struct Accounts {
    inner: Arc<Inner>,
}

struct Inner {
    base: Proton,
    root: PathBuf,
    key: SymmetricKey,
    registry: Mutex<Registry>,
}

struct Registry {
    list: AccountList,
    open: HashMap<UserId, Proton>,
}

impl Accounts {
    pub(crate) async fn open(base: Proton, root: PathBuf, key: SymmetricKey) -> Result<Accounts> {
        let list = {
            let (root, key) = (root.clone(), key.clone());
            offload(move || -> Result<AccountList> {
                private_file::create_private_dir(&root).map_err(|source| Error::Storage {
                    path: root.clone(),
                    source,
                })?;
                let sealed =
                    SealedFile::new(root.join(ACCOUNT_LIST_FILE), key, ACCOUNT_LIST_PURPOSE);
                match sealed.read::<AccountList>() {
                    Ok(list) => Ok(list.unwrap_or_default()),
                    Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                        Err(Error::InvalidAccountList(sealed.path().to_path_buf()))
                    }
                    Err(source) => Err(Error::Storage {
                        path: sealed.path().to_path_buf(),
                        source,
                    }),
                }
            })
            .await?
        };

        Ok(Accounts {
            inner: Arc::new(Inner {
                base,
                root,
                key,
                registry: Mutex::new(Registry {
                    list,
                    open: HashMap::new(),
                }),
            }),
        })
    }

    #[must_use]
    pub fn list(&self) -> Vec<AccountInfo> {
        let registry = self.registry();
        let primary = registry.list.primary.as_ref();
        registry
            .list
            .accounts
            .iter()
            .filter(|account| account.state != AccountState::Removed)
            .map(|account| AccountInfo {
                user: account.user.clone(),
                name: account.name.clone(),
                email: account.email.clone(),
                state: account.state,
                primary: primary == Some(&account.user),
            })
            .collect()
    }

    #[must_use]
    pub fn primary(&self) -> Option<UserId> {
        self.registry().list.primary.clone()
    }

    #[must_use]
    pub fn state(&self, user: &UserId) -> AccountState {
        self.registry()
            .list
            .get(user)
            .map_or(AccountState::Removed, |account| account.state)
    }

    pub async fn set_primary(&self, user: &UserId) -> Result<()> {
        let list = {
            let mut registry = self.registry();
            if !registry.list.is_ready(user) {
                return Err(Error::NoSuchAccount(user.clone()));
            }
            registry.list.primary = Some(user.clone());
            registry.list.clone()
        };
        self.persist(list).await
    }

    pub fn signing_in(&self) -> Result<Proton> {
        let account = self.inner.base.account();
        let api = account
            .api()
            .with_session_store(MemorySessionStore::new())?;
        Ok(Proton::assembled(
            api,
            *account.pgp(),
            self.inner.base.log_lists().clone(),
        ))
    }

    pub async fn adopt(&self, session: Session) -> Result<Session> {
        let keys = AccountKeys::clone(session.keys());
        let user = keys.user().id().clone();
        let api_session = session
            .proton()
            .account()
            .api()
            .session()
            .ok_or(rikka_proton_api::Error::MissingSession)?
            .with_user_id(user.as_str());

        let proton = self.open_account(&user).await?;
        let api = proton.account().api().clone();
        offload(move || api.set_session(api_session)).await?;

        let list = {
            let mut registry = self.registry();
            let account = StoredAccount {
                user: user.clone(),
                name: keys.user().name().map(str::to_owned),
                email: keys
                    .primary_address()
                    .ok()
                    .map(|address| address.email().to_owned())
                    .or_else(|| keys.user().email().map(str::to_owned)),
                state: AccountState::Ready,
            };
            match registry
                .list
                .accounts
                .iter_mut()
                .find(|stored| stored.user == user)
            {
                Some(stored) => *stored = account,
                None => registry.list.accounts.push(account),
            }
            registry.list.ensure_primary();
            registry.open.insert(user.clone(), proton.clone());
            registry.list.clone()
        };
        self.persist(list).await?;

        Ok(proton.unlocked(keys))
    }

    pub async fn proton(&self, user: &UserId) -> Result<Proton> {
        {
            let registry = self.registry();
            if !registry.list.is_ready(user) {
                return Err(Error::NoSuchAccount(user.clone()));
            }
            if let Some(proton) = registry.open.get(user) {
                return Ok(proton.clone());
            }
        }
        let proton = self.open_account(user).await?;
        Ok(self
            .registry()
            .open
            .entry(user.clone())
            .or_insert(proton)
            .clone())
    }

    pub async fn resume(&self, user: &UserId, passphrase: impl AsRef<[u8]>) -> Result<Session> {
        self.proton(user).await?.resume_cached(passphrase).await
    }

    pub async fn disable(&self, user: &UserId) -> Result<()> {
        let signed_in = match self.state(user) {
            AccountState::Ready => Some(self.proton(user).await?),
            AccountState::Disabled => None,
            AccountState::Removed => return Err(Error::NoSuchAccount(user.clone())),
        };
        if let Some(proton) = signed_in {
            if let Err(error) = proton.sign_out().await {
                tracing::warn!(%error, "failed to revoke session, removing locally");
            }
            if let Some(cache) = proton.account().cache().cloned() {
                offload(move || cache.retire()).await?;
            }
        }

        let list = {
            let mut registry = self.registry();
            registry.open.remove(user);
            if let Some(account) = registry
                .list
                .accounts
                .iter_mut()
                .find(|account| &account.user == user)
            {
                account.state = AccountState::Disabled;
            }
            registry.list.ensure_primary();
            registry.list.clone()
        };
        self.persist(list).await
    }

    pub async fn remove(&self, user: &UserId) -> Result<()> {
        if self.state(user) == AccountState::Ready {
            self.disable(user).await?;
        }
        let dir = self.account_dir(user);
        offload(move || match fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(Error::Storage { path: dir, source }),
        })
        .await?;

        let list = {
            let mut registry = self.registry();
            registry
                .list
                .accounts
                .retain(|account| &account.user != user);
            registry.list.ensure_primary();
            registry.list.clone()
        };
        self.persist(list).await
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.inner.root
    }

    async fn open_account(&self, user: &UserId) -> Result<Proton> {
        let dir = self.account_dir(user);
        let key = self.account_key(user)?;
        let base = self.inner.base.clone();
        offload(move || -> Result<Proton> {
            private_file::create_private_dir(&dir).map_err(|source| Error::Storage {
                path: dir.clone(),
                source,
            })?;
            let sessions = SealedSessionStore::new(SealedFile::new(
                dir.join(SESSION_FILE),
                key.clone(),
                SESSION_PURPOSE,
            ));
            let account = base.account();
            let api = account.api().with_session_store(sessions)?;
            let cache = Cache::open(dir.join(CACHE_DIR), key)?;
            Ok(Proton::assembled(api, *account.pgp(), base.log_lists().clone()).with_cache(cache))
        })
        .await
    }

    async fn persist(&self, list: AccountList) -> Result<()> {
        let path = self.inner.root.join(ACCOUNT_LIST_FILE);
        let key = self.inner.key.clone();
        offload(move || {
            SealedFile::new(path.clone(), key, ACCOUNT_LIST_PURPOSE)
                .write(&list)
                .map_err(|source| Error::Storage { path, source })
        })
        .await
    }

    fn account_dir(&self, user: &UserId) -> PathBuf {
        let digest = sha256(user.as_str().as_bytes());
        self.inner
            .root
            .join(rikka_types::hex::encode(&digest[..ACCOUNT_DIR_HASH_LEN]))
    }

    fn account_key(&self, user: &UserId) -> Result<SymmetricKey> {
        let derived = hmac(
            Hash::Sha256,
            self.inner.key.as_bytes(),
            format!("{ACCOUNT_KEY_CONTEXT}{}", user.as_str()),
        );
        Ok(SymmetricKey::from_bytes(derived)?)
    }

    fn registry(&self) -> MutexGuard<'_, Registry> {
        self.inner.registry.lock()
    }
}

impl fmt::Debug for Accounts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Accounts")
            .field("root", &self.inner.root)
            .field("accounts", &self.registry().list.accounts.len())
            .finish_non_exhaustive()
    }
}
