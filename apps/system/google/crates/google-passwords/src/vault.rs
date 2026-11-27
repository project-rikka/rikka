use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use google_account::Grants;
use google_sync::{Engine, Lock, PassphraseType, Refresh, Server, now_ms};
use rikka_sync::Mutex;

use crate::credential::{Credential, Draft};
use crate::error::{Error, Result};
use crate::seal::Seal;

const FILE: &str = "passwords.sealed";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    pub lock: Lock,
    pub passphrase_type: Option<PassphraseType>,
    pub has_synced: bool,
    pub unsynced: usize,
    pub undecryptable: usize,
    pub revision: u64,
}

#[derive(Clone, Debug)]
pub struct Vault {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    grants: Arc<Grants>,
    server: Server,
    engine: Mutex<Engine>,
    writing: Mutex<()>,
    turn: tokio::sync::Mutex<()>,
    seal: Seal,
    path: PathBuf,
}

impl Vault {
    pub fn open(folder: &Path, grants: Arc<Grants>, server: Server, seal: Seal) -> Result<Vault> {
        let path = folder.join(FILE);
        let account = grants.master().email().to_owned();
        let engine = match std::fs::read(&path) {
            Ok(sealed) => restored(&account, &seal, &sealed),
            Err(error) if error.kind() == ErrorKind::NotFound => Engine::new(&account),
            Err(error) => return Err(Error::Read(error)),
        };
        Ok(Vault {
            inner: Arc::new(Inner {
                grants,
                server,
                engine: Mutex::new(engine),
                writing: Mutex::new(()),
                turn: tokio::sync::Mutex::new(()),
                seal,
                path,
            }),
        })
    }

    #[must_use]
    pub fn status(&self) -> Status {
        let engine = self.inner.engine.lock();
        Status {
            lock: engine.lock(),
            passphrase_type: engine.passphrase_type(),
            has_synced: engine.has_synced(),
            unsynced: engine.unsynced(),
            undecryptable: engine.undecryptable(),
            revision: engine.revision(),
        }
    }

    #[must_use]
    pub fn poll_interval(&self) -> Duration {
        self.inner.engine.lock().poll_interval()
    }

    #[must_use]
    pub fn credentials(&self) -> Vec<Credential> {
        let engine = self.inner.engine.lock();
        let mut credentials: Vec<Credential> = engine
            .passwords()
            .filter_map(|(id, data)| Credential::from_specifics(id, data))
            .collect();
        credentials.sort_by_cached_key(|credential| {
            (
                credential.site.group.to_lowercase(),
                credential.site.name.to_lowercase(),
                credential.username.to_lowercase(),
            )
        });
        credentials
    }

    pub fn save(&self, draft: &Draft, replacing: Option<&str>) -> Result<String> {
        let id = {
            let mut engine = self.inner.engine.lock();
            let base = match replacing {
                Some(id) => Some(engine.password(id).ok_or(Error::Missing)?.clone()),
                None => None,
            };
            let data = draft.specifics(base.as_ref(), now_ms())?;
            engine.put(data, replacing)
        };
        self.persist()?;
        Ok(id)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        self.inner.engine.lock().remove(id);
        self.persist()
    }

    pub fn offer_passphrase(&self, passphrase: &str) -> Result<bool> {
        let accepted = self.inner.engine.lock().offer_passphrase(passphrase)?;
        if accepted {
            self.persist()?;
        }
        Ok(accepted)
    }

    pub fn offer_vault_keys(&self, keys: &[Vec<u8>]) -> Result<bool> {
        let accepted = self.inner.engine.lock().offer_vault_keys(keys)?;
        if accepted {
            self.persist()?;
        }
        Ok(accepted)
    }

    pub async fn sync(&self, refresh: Option<Refresh>) -> Result<Status> {
        let _turn = self.inner.turn.lock().await;
        if let Some(refresh) = refresh {
            self.inner.engine.lock().request_refresh(refresh);
        }
        let mut restarted = false;
        loop {
            let asked = {
                let mut engine = self.inner.engine.lock();
                engine
                    .next_request()
                    .map(|message| (message, engine.cache_guid().to_owned()))
            };
            let Some((message, cache_guid)) = asked else {
                break;
            };
            let answer = self
                .inner
                .server
                .exchange(&self.inner.grants, &cache_guid, &message)
                .await;
            let received = match answer {
                Ok(answer) => self.inner.engine.lock().receive(answer),
                Err(error) => {
                    self.inner.engine.lock().abandon();
                    return Err(error.into());
                }
            };
            self.persist()?;
            match received {
                Ok(()) => {}
                Err(google_sync::Error::StoreReset) if !restarted => restarted = true,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(self.status())
    }

    fn persist(&self) -> Result<()> {
        let _writing = self.inner.writing.lock();
        let snapshot = zeroize::Zeroizing::new(self.inner.engine.lock().snapshot());
        let sealed = self.inner.seal.seal(&snapshot);
        rikka_account_book::write_private(&self.inner.path, &sealed)?;
        Ok(())
    }
}

fn restored(account: &str, seal: &Seal, sealed: &[u8]) -> Engine {
    let restored = seal
        .open(sealed)
        .and_then(|snapshot| Ok(Engine::restore(account, &snapshot)?));
    match restored {
        Ok(engine) => engine,
        Err(error) => {
            tracing::warn!("starting password sync over: {error}");
            Engine::new(account)
        }
    }
}
