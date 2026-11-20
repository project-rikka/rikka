use std::fs::File;
use std::sync::Arc;

use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use rikka_llm_adapter::{SharedTokens, TokenStore};
use serde_json::Value;

use crate::backend::Backend;
use crate::credential::Credential;

pub trait CredentialStore: Send + Sync + 'static {
    fn lock(&self, backend: Backend) -> BoxFuture<'_, Option<File>>;

    fn load(&self, backend: Backend) -> BoxFuture<'_, Option<Credential>>;

    fn save(&self, backend: Backend, credential: Credential) -> BoxFuture<'_, ()>;
}

#[derive(Clone, Default)]
pub struct Renewal {
    shared: Option<Arc<[Arc<SharedTokens>]>>,
}

impl Renewal {
    #[must_use]
    pub fn new(store: impl CredentialStore) -> Renewal {
        let store: Arc<dyn CredentialStore> = Arc::new(store);
        let shared = Backend::all()
            .map(|backend| {
                Arc::new(SharedTokens::new(Arc::new(Kept {
                    backend,
                    store: Arc::clone(&store),
                })))
            })
            .collect();
        Renewal {
            shared: Some(shared),
        }
    }

    #[must_use]
    pub fn none() -> Renewal {
        Renewal::default()
    }

    pub fn hold(&self, backend: Backend, credential: Option<&Credential>) {
        if let Some(shared) = self.shared(backend) {
            let tokens = credential
                .filter(|credential| credential.backend() == Some(backend))
                .and_then(Credential::tokens);
            shared.hold(tokens.cloned());
        }
    }

    pub(crate) fn shared(&self, backend: Backend) -> Option<Arc<SharedTokens>> {
        self.shared
            .as_ref()
            .map(|shared| Arc::clone(&shared[backend.index()]))
    }
}

struct Kept {
    backend: Backend,
    store: Arc<dyn CredentialStore>,
}

impl TokenStore for Kept {
    fn lock(&self) -> BoxFuture<'_, Option<File>> {
        self.store.lock(self.backend)
    }

    fn load(&self) -> BoxFuture<'_, Option<Value>> {
        self.store
            .load(self.backend)
            .map(|loaded| {
                loaded
                    .filter(|credential| credential.backend() == Some(self.backend))
                    .and_then(|credential| credential.tokens().cloned())
            })
            .boxed()
    }

    fn save(&self, tokens: Value) -> BoxFuture<'_, ()> {
        self.store
            .save(self.backend, Credential::signed(self.backend, tokens))
    }
}

impl std::fmt::Debug for Renewal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Renewal")
            .field("shared", &self.shared.is_some())
            .finish()
    }
}
