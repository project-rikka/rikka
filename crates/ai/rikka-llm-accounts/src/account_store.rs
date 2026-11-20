use rikka_sync::{Mutex, MutexGuard};
use std::collections::HashMap;
use std::fs::{DirBuilder, File, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use rikka_llm::ModelInfo;
use rikka_llm_providers::{
    Backend, Credential, CredentialStore, Endpoint, EndpointCredentials, Renewal,
};
use rikka_secrets::blocking::Secrets;
use rikka_secrets::{Entry, schema};
use rikka_settings::seam::Settings;
use tokio::sync::Notify;

use crate::account::Account;
use crate::account_id::{self, AccountId, Kind};

pub(crate) const BACKENDS: &str = "backends";
pub(crate) const ENDPOINTS: &str = "endpoints";

const BACKEND: &str = "backend";
const ENDPOINT: &str = "endpoint";
const LOCKS: &str = "llm";
const LOCKS_MODE: u32 = 0o700;
const LOCK_MODE: u32 = 0o600;

#[derive(Clone, Debug)]
pub(crate) struct AccountStore {
    settings: Arc<Mutex<Settings>>,
    renewed: Arc<AtomicU64>,
    renewal: Renewal,
    listing: Arc<Notify>,
}

#[derive(Debug)]
pub(crate) struct Loaded {
    pub(crate) accounts: Arc<Vec<Account>>,
    pub(crate) fault: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct StoredBackend {
    backend: String,
    #[serde(default)]
    model: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    models: Vec<ModelInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    listed: Option<u64>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct StoredEndpoint {
    id: AccountId,
    name: String,
    endpoint: Endpoint,
    #[serde(default)]
    model: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    models: Vec<ModelInfo>,
}

#[derive(Default)]
struct Kept {
    backends: HashMap<Backend, Credential>,
    endpoints: HashMap<AccountId, EndpointCredentials>,
}

pub(crate) enum Secret<'a> {
    Backend(Option<&'a Credential>),
    Endpoint(Option<&'a EndpointCredentials>),
}

impl AccountStore {
    pub(crate) fn new(settings: Settings) -> AccountStore {
        let renewed = Arc::<AtomicU64>::default();
        AccountStore {
            settings: Arc::new(Mutex::new(settings)),
            renewal: Renewal::new(Keyring {
                renewed: Arc::clone(&renewed),
            }),
            renewed,
            listing: Arc::default(),
        }
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Settings> {
        self.settings.lock()
    }

    pub(crate) fn renewals(&self) -> u64 {
        self.renewed.load(Ordering::Acquire)
    }

    pub(crate) fn wake_listing(&self) {
        self.listing.notify_one();
    }

    pub(crate) async fn listing_woken(&self, within: Duration) {
        tokio::select! {
            () = self.listing.notified() => {}
            () = tokio::time::sleep(within) => {}
        }
    }

    pub(crate) fn load(&self) -> Loaded {
        let mut accounts = {
            let settings = self.lock();
            listed(
                settings.read(BACKENDS).unwrap_or_default(),
                settings.read(ENDPOINTS).unwrap_or_default(),
            )
        };
        let fault = match credentials() {
            Ok(mut kept) => {
                for account in &mut accounts {
                    match account.backend() {
                        Some(backend) => {
                            account.set_credential(kept.backends.remove(&backend));
                            self.renewal.hold(backend, account.credential());
                        }
                        None => account.set_endpoint_credentials(
                            kept.endpoints.remove(&account.id()).unwrap_or_default(),
                        ),
                    }
                }
                None
            }
            Err(error) => {
                tracing::warn!("failed to read model credentials from the keyring: {error}");
                Some(error.to_string())
            }
        };
        Loaded {
            accounts: Arc::new(accounts),
            fault,
        }
    }

    pub(crate) fn keep(&self, account: &Account, secret: Secret<'_>) -> Result<(), String> {
        kept(account.id(), account.label(), &secret)?;
        if let (Some(backend), Secret::Backend(credential)) = (account.backend(), secret) {
            self.renewal.hold(backend, credential);
        }
        Ok(())
    }

    pub(crate) fn save(&self, accounts: &[Account]) {
        let backends: Vec<StoredBackend> = accounts
            .iter()
            .filter_map(|account| {
                Some(StoredBackend {
                    backend: account.backend()?.id().to_owned(),
                    model: account.model().to_owned(),
                    models: account.kept_models().to_vec(),
                    listed: account.listed(),
                })
            })
            .collect();
        let endpoints: Vec<StoredEndpoint> = accounts
            .iter()
            .filter_map(|account| {
                Some(StoredEndpoint {
                    id: account.id(),
                    name: account.label().to_owned(),
                    endpoint: account.endpoint()?.clone(),
                    model: account.model().to_owned(),
                    models: account.kept_models().to_vec(),
                })
            })
            .collect();
        let mut settings = self.lock();
        settings.set_or_warn(BACKENDS, &backends);
        settings.set_or_warn(ENDPOINTS, &endpoints);
    }

    pub(crate) fn renewal(&self) -> Renewal {
        self.renewal.clone()
    }
}

pub(crate) fn readable(settings: &Settings) -> Result<(), String> {
    settings
        .get::<Vec<StoredBackend>>(BACKENDS)
        .map_err(|error| format!("{BACKENDS}: {error}"))?;
    settings
        .get::<Vec<StoredEndpoint>>(ENDPOINTS)
        .map_err(|error| format!("{ENDPOINTS}: {error}"))?;
    Ok(())
}

pub(crate) fn filled() -> Vec<Account> {
    listed(Vec::new(), Vec::new())
}

fn listed(backends: Vec<StoredBackend>, endpoints: Vec<StoredEndpoint>) -> Vec<Account> {
    let mut stored: HashMap<Backend, StoredBackend> = backends
        .into_iter()
        .filter_map(|stored| Some((Backend::from_id(&stored.backend)?, stored)))
        .collect();
    Backend::all()
        .map(|backend| match stored.remove(&backend) {
            Some(kept) => Account::kept(backend, kept.model, kept.models, kept.listed),
            None => Account::new(backend),
        })
        .chain(
            endpoints
                .into_iter()
                .filter(|stored| stored.id.is_endpoint())
                .map(|stored| {
                    Account::from_endpoint(
                        stored.id,
                        stored.name,
                        stored.endpoint,
                        stored.model,
                        stored.models,
                    )
                }),
        )
        .collect()
}

struct Keyring {
    renewed: Arc<AtomicU64>,
}

impl CredentialStore for Keyring {
    fn lock(&self, backend: Backend) -> BoxFuture<'_, Option<File>> {
        blocking(move || locked(backend)).boxed()
    }

    fn load(&self, backend: Backend) -> BoxFuture<'_, Option<Credential>> {
        blocking(move || {
            Secrets::shared()
                .and_then(|secrets| secrets.lookup_json(&entry(AccountId::from(backend))))
                .inspect_err(|error| {
                    tracing::warn!(
                        "failed to read stored credentials for {}: {error}",
                        backend.label()
                    );
                })
                .ok()
                .flatten()
        })
        .boxed()
    }

    fn save(&self, backend: Backend, credential: Credential) -> BoxFuture<'_, ()> {
        async move {
            blocking(move || {
                kept(
                    AccountId::from(backend),
                    backend.label(),
                    &Secret::Backend(Some(&credential)),
                )
                .inspect_err(|error| {
                    tracing::warn!(
                        "failed to save renewed credentials for {}: {error}",
                        backend.label()
                    );
                })
                .ok()
            })
            .await;
            self.renewed.fetch_add(1, Ordering::Release);
        }
        .boxed()
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Option<T> + Send + 'static,
) -> Option<T> {
    tokio::task::spawn_blocking(work)
        .await
        .inspect_err(|error| tracing::warn!("failed to reach the keyring: {error}"))
        .ok()
        .flatten()
}

fn locked(backend: Backend) -> Option<File> {
    let paths = rikka_dirs::Paths::resolve().ok()?;
    let folder = paths.runtime_dir()?.join(LOCKS);
    DirBuilder::new()
        .recursive(true)
        .mode(LOCKS_MODE)
        .create(&folder)
        .and_then(|()| {
            OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .mode(LOCK_MODE)
                .open(folder.join(format!("{}.lock", backend.id())))
        })
        .and_then(|file| file.lock().map(|()| file))
        .inspect_err(|error| {
            tracing::warn!("failed to lock the renewal of {}: {error}", backend.label());
        })
        .ok()
}

fn kept(id: AccountId, label: &str, secret: &Secret<'_>) -> Result<(), String> {
    let secrets = Secrets::shared().map_err(|error| error.to_string())?;
    let entry = entry(id);
    let label = format!("{label} sign-in");
    let stored = match secret {
        Secret::Backend(Some(credential)) => secrets.store_json(&entry, &label, credential),
        Secret::Endpoint(Some(credentials)) if !credentials.is_empty() => {
            secrets.store_json(&entry, &label, credentials)
        }
        Secret::Backend(None) | Secret::Endpoint(_) => secrets.remove(&entry).map(|_| ()),
    };
    stored.map_err(|error| error.to_string())
}

fn credentials() -> rikka_secrets::Result<Kept> {
    let found = Secrets::shared()?.search(&Entry::new(schema::LLM))?;
    let mut kept = Kept::default();
    for found in found {
        let secret = found.secret.expose();
        if let Some(backend) = found
            .attributes
            .get(BACKEND)
            .and_then(|id| Backend::from_id(id))
        {
            match serde_json::from_slice::<Credential>(secret) {
                Ok(credential) => {
                    kept.backends.insert(backend, credential);
                }
                Err(error) => tracing::warn!(
                    "ignoring unreadable credentials for {}: {error}",
                    backend.label()
                ),
            }
        } else if let Some(id) = found
            .attributes
            .get(ENDPOINT)
            .and_then(|number| AccountId::from_endpoint_number(number))
        {
            match serde_json::from_slice::<EndpointCredentials>(secret) {
                Ok(credentials) => {
                    kept.endpoints.insert(id, credentials);
                }
                Err(error) => tracing::warn!("ignoring unreadable credentials for {id}: {error}"),
            }
        }
    }
    Ok(kept)
}

fn entry(id: AccountId) -> Entry {
    let entry = Entry::new(schema::LLM);
    match id.kind() {
        Kind::Backend(backend) => entry.attribute(BACKEND, backend.id()),
        Kind::Endpoint(number) => entry.attribute(ENDPOINT, account_id::hex(number)),
    }
}
