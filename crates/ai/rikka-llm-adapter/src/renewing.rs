use rikka_sync::{Mutex, MutexGuard};
use std::fs::File;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::future::BoxFuture;
use rikka_llm::wire::{Authorize, Call, Transport};
use rikka_llm::{Error, Result};
use rikka_net::{HeaderMap, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

const RENEWAL_MARGIN: Duration = Duration::from_secs(300);

pub trait Credentials: rikka_oauth::Refresh<Error = rikka_llm::Error> + std::fmt::Debug {
    fn headers(&self) -> Result<HeaderMap>;
}

pub trait TokenStore: Send + Sync + 'static {
    fn lock(&self) -> BoxFuture<'_, Option<File>>;

    fn load(&self) -> BoxFuture<'_, Option<Value>>;

    fn save(&self, tokens: Value) -> BoxFuture<'_, ()>;
}

pub struct SharedTokens {
    held: Mutex<Held>,
    renewing: futures_util::lock::Mutex<()>,
    store: Option<Arc<dyn TokenStore>>,
}

#[derive(Default)]
struct Held {
    tokens: Option<Value>,
    renewed: bool,
}

impl SharedTokens {
    #[must_use]
    pub fn new(store: Arc<dyn TokenStore>) -> SharedTokens {
        SharedTokens {
            held: Mutex::default(),
            renewing: futures_util::lock::Mutex::new(()),
            store: Some(store),
        }
    }

    pub(crate) fn private(tokens: Value) -> SharedTokens {
        SharedTokens {
            held: Mutex::new(Held {
                tokens: Some(tokens),
                renewed: false,
            }),
            renewing: futures_util::lock::Mutex::new(()),
            store: None,
        }
    }

    pub fn hold(&self, tokens: Option<Value>) {
        let mut held = self.held();
        if held.tokens != tokens {
            *held = Held {
                tokens,
                renewed: false,
            };
        }
    }

    pub(crate) fn offer(&self, tokens: Value) {
        let mut held = self.held();
        if held.tokens.is_none() {
            held.tokens = Some(tokens);
        }
    }

    fn renew(&self, tokens: Value) {
        *self.held() = Held {
            tokens: Some(tokens),
            renewed: true,
        };
    }

    fn held(&self) -> MutexGuard<'_, Held> {
        self.held.lock()
    }
}

impl std::fmt::Debug for SharedTokens {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SharedTokens")
            .field("stored", &self.store.is_some())
            .finish_non_exhaustive()
    }
}

pub(crate) fn renewing<T>(
    endpoint: Url,
    headers: HeaderMap,
    shared: Arc<SharedTokens>,
) -> Result<Transport>
where
    T: Credentials + Serialize + DeserializeOwned,
{
    let signing: Signing<T> = Signing {
        shared,
        credentials: PhantomData,
    };
    Transport::authorized(endpoint, headers, Arc::new(signing))
}

struct Signing<T> {
    shared: Arc<SharedTokens>,
    credentials: PhantomData<fn() -> T>,
}

struct Reading<T> {
    tokens: T,
    written: Value,
    fresh: bool,
}

impl<T: Credentials + Serialize + DeserializeOwned> Signing<T> {
    async fn signed(&self) -> Result<HeaderMap> {
        if let Some(held) = self.held().filter(|held| held.fresh) {
            return held.tokens.headers();
        }

        let _renewing = self.shared.renewing.lock().await;
        let held = self.held();
        if let Some(held) = held.as_ref().filter(|held| held.fresh) {
            return held.tokens.headers();
        }

        let (_locked, stored) = match &self.shared.store {
            Some(store) => (store.lock().await, store.load().await),
            None => (None, None),
        };
        let stored = stored.and_then(|written| {
            let tokens = T::deserialize(&written).ok()?;
            let renewed_elsewhere = held.as_ref().is_none_or(|held| held.written != written);
            let fresh = is_fresh(&tokens, renewed_elsewhere);
            Some(Reading {
                tokens,
                written,
                fresh,
            })
        });
        if let Some(stored) = stored.as_ref().filter(|stored| stored.fresh) {
            self.shared.renew(stored.written.clone());
            return stored.tokens.headers();
        }

        let stale = stored.or(held).ok_or(Error::Expired)?;
        let renewed = stale.tokens.refresh().await?;
        let written = serde_json::to_value(&renewed).expect("credentials should serialize to JSON");
        if let Some(store) = &self.shared.store {
            store.save(written.clone()).await;
        }
        self.shared.renew(written);
        renewed.headers()
    }

    fn held(&self) -> Option<Reading<T>> {
        let held = self.shared.held();
        let written = held.tokens.clone()?;
        let tokens = T::deserialize(&written).ok()?;
        let fresh = is_fresh(&tokens, held.renewed);
        Some(Reading {
            tokens,
            written,
            fresh,
        })
    }
}

fn is_fresh(tokens: &impl rikka_oauth::Refresh, renewed: bool) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    match tokens.expires_at() {
        Some(expires_at) => now + RENEWAL_MARGIN.as_secs() < expires_at,
        None => renewed,
    }
}

impl<T> std::fmt::Debug for Signing<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Signing")
            .field("shared", &self.shared)
            .finish()
    }
}

impl<T: Credentials + Serialize + DeserializeOwned> Authorize for Signing<T> {
    fn headers(&self) -> Call<'_, HeaderMap> {
        Box::pin(self.signed())
    }
}
