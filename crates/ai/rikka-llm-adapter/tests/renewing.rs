use rikka_sync::Mutex;
use std::fs::File;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use rikka_llm::wire::{self, Messages};
use rikka_llm::{Model, ModelInfo, Value, json};
use rikka_llm_adapter::{
    Access, Adapter, Auth, Credentials, DynAdapter, Result, Service, SharedTokens, SignInMethod,
    TokenStore,
};
use rikka_net::HeaderMap;
use rikka_oauth::Refresh;
use rikka_test_llm::Server;
use serde::{Deserialize, Serialize};

const EXPIRED: u64 = 1;
const LASTING: u64 = u64::MAX / 2;

static RENEWALS: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Tokens {
    refresh_token: String,
    expires_at: u64,
}

impl Refresh for Tokens {
    type Error = rikka_llm::Error;

    fn expires_at(&self) -> Option<u64> {
        Some(self.expires_at)
    }

    async fn refresh(&self) -> rikka_llm::Result<Tokens> {
        RENEWALS.fetch_add(1, Ordering::SeqCst);
        tokio::task::yield_now().await;
        Ok(Tokens {
            refresh_token: format!("{}'", self.refresh_token),
            expires_at: LASTING,
        })
    }
}

impl Credentials for Tokens {
    fn headers(&self) -> rikka_llm::Result<HeaderMap> {
        Ok(HeaderMap::new())
    }
}

struct Signed {
    endpoint: String,
}

impl Adapter for Signed {
    type Tokens = Tokens;

    const SERVICE: Service<Tokens> = Service {
        id: "signed",
        label: "Signed",
        sign_in: SignInMethod::Key { variable: "SIGNED" },
    };

    fn connect(&self, auth: &Auth<'_, Tokens>, model: &ModelInfo) -> Result<Model> {
        let transport = auth.transport(wire::endpoint(&self.endpoint)?, HeaderMap::new())?;
        Ok(Messages::new(transport, model.id()).build())
    }
}

#[derive(Default)]
struct Kept {
    tokens: Mutex<Option<Value>>,
    saves: AtomicUsize,
}

impl TokenStore for Kept {
    fn lock(&self) -> BoxFuture<'_, Option<File>> {
        async { None }.boxed()
    }

    fn load(&self) -> BoxFuture<'_, Option<Value>> {
        async { self.tokens.lock().clone() }.boxed()
    }

    fn save(&self, tokens: Value) -> BoxFuture<'_, ()> {
        async move {
            self.saves.fetch_add(1, Ordering::SeqCst);
            *self.tokens.lock() = Some(tokens);
        }
        .boxed()
    }
}

fn tokens(expires_at: u64) -> Value {
    json!({ "refresh_token": "first", "expires_at": expires_at })
}

async fn answered(model: &Model) -> bool {
    match model.chat().user("hi").stream().await {
        Ok(stream) => stream.response().await.is_ok(),
        Err(_) => false,
    }
}

#[tokio::test]
async fn transports_of_one_account_renew_once() {
    let server = Server::streaming(&[
        r#"{"type":"message_start","message":{"id":"msg_1","model":"m","usage":{}}}"#,
        r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{}}"#,
        r#"{"type":"message_stop"}"#,
    ]);
    let adapter = Signed {
        endpoint: server.url().to_owned(),
    };
    let expired = tokens(EXPIRED);

    let kept = Arc::new(Kept::default());
    let shared = Arc::new(SharedTokens::new(Arc::clone(&kept) as Arc<dyn TokenStore>));
    shared.hold(Some(expired.clone()));
    let connect = |shared: &Arc<SharedTokens>| {
        let access = Access::new(String::new(), Some(&expired), Some(Arc::clone(shared)));
        DynAdapter::connect(&adapter, &access, &ModelInfo::new("m", "M", 0, 0))
            .expect("the account is signed in")
    };
    let (one, other) = (connect(&shared), connect(&shared));

    let (first, second) = tokio::join!(answered(&one), answered(&other));
    assert!(first && second);
    assert_eq!(RENEWALS.load(Ordering::SeqCst), 1);
    assert_eq!(kept.saves.load(Ordering::SeqCst), 1);

    let renewed_elsewhere = Arc::new(Kept {
        tokens: Mutex::new(Some(tokens(LASTING))),
        saves: AtomicUsize::new(0),
    });
    let shared = Arc::new(SharedTokens::new(
        Arc::clone(&renewed_elsewhere) as Arc<dyn TokenStore>
    ));
    shared.hold(Some(expired.clone()));
    assert!(answered(&connect(&shared)).await);
    assert_eq!(RENEWALS.load(Ordering::SeqCst), 1);
    assert_eq!(renewed_elsewhere.saves.load(Ordering::SeqCst), 0);
}
