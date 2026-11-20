use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rikka_proton_api::{Environment, Session};
use rikka_proton_client::Proton;
use rikka_proton_pgp::{Pgp, Srp};
use rikka_test_proton::{EMAIL, Request, ResponseFuture, json_response, lock_key, new_key, serve};
use serde_json::json;

use proton_session::{Access, AccountStore};

pub(crate) const PASSWORD: &str = "the account password";
const SALT: &str = "jQ0LTS8bBqQQZZL4MzmtQQ==";

#[derive(Clone)]
pub(crate) struct Account {
    user_key: Arc<String>,
    requests: Arc<AtomicUsize>,
    signed_out: Arc<AtomicBool>,
    used_space: Arc<AtomicUsize>,
}

impl Account {
    pub(crate) fn new() -> Account {
        let user_key = lock_key(&new_key(&Pgp::new(), EMAIL), passphrase());

        Account {
            user_key: Arc::new(user_key),
            requests: Arc::new(AtomicUsize::new(0)),
            signed_out: Arc::new(AtomicBool::new(false)),
            used_space: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub(crate) fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }

    pub(crate) fn change_description(&self) {
        self.used_space.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn sign_out(&self) {
        self.signed_out.store(true, Ordering::SeqCst);
    }

    pub(crate) async fn access(&self, store: &AccountStore) -> Access {
        let account = self.clone();
        let address = serve(move |request: Request| account.respond(&request)).await;
        let proton = Proton::builder()
            .app_version(proton_session::APP_VERSION)
            .environment(Environment::custom(&address).expect("invalid server URL"))
            .session_store(store.sessions())
            .build()
            .expect("failed to build client");
        let api = proton.account().api();
        if api.session().is_none() {
            api.set_session(Session::new("uid", "access", "refresh"))
                .expect("failed to store session");
        }

        Access::with_proton(proton, store.clone())
    }

    fn respond(&self, request: &Request) -> ResponseFuture {
        self.requests.fetch_add(1, Ordering::SeqCst);
        if self.signed_out.load(Ordering::SeqCst) {
            return json_response(401, &json!({"Code": 401}));
        }

        let body = match request.route() {
            ("GET", "/core/v4/users") => json!({
                "Code": 1000,
                "User": {
                    "ID": "user-value",
                    "Name": "tester",
                    "UsedSpace": self.used_space.load(Ordering::SeqCst),
                    "Keys": [{
                        "ID": "user-key",
                        "Version": 3,
                        "PrivateKey": *self.user_key,
                        "Primary": 1,
                        "Active": 1
                    }]
                }
            }),
            ("GET", "/core/v4/addresses") => json!({"Code": 1000, "Total": 0, "Addresses": []}),
            ("GET", "/core/v4/keys/salts") => {
                json!({"Code": 1000, "KeySalts": [{"ID": "user-key", "KeySalt": SALT}]})
            }
            _ => return json_response(404, &json!({"Code": 2501})),
        };
        json_response(200, &body)
    }
}

fn passphrase() -> Vec<u8> {
    let salt = rikka_proton_api::base64::decode(SALT).expect("invalid salt");

    Srp::new()
        .mailbox_passphrase(PASSWORD, salt)
        .expect("failed to derive passphrase")
        .as_bytes()
        .to_vec()
}
