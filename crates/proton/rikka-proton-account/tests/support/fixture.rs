use std::sync::{Arc, Mutex};

use rikka_proton_account::Account;
use rikka_proton_pgp::{Pgp, PrivateKey, Srp, random_hex};
use serde_json::{Value, json};

use crate::support::account;
use rikka_test_proton::{
    FakeSrp, Request, ResponseFuture, SIGNED_MODULUS, lock_key, not_found, response,
};

pub(crate) const PASSWORD: &str = "the account password";
pub(crate) const SALT: &str = "jQ0LTS8bBqQQZZL4MzmtQQ==";
pub(crate) const EMAIL: &str = "tester@proton.test";

pub(crate) fn passphrase(password: &str, salt: &str) -> Vec<u8> {
    let salt = rikka_proton_api::base64::decode(salt).unwrap();

    Srp::new()
        .mailbox_passphrase(password, salt)
        .unwrap()
        .as_bytes()
        .to_vec()
}

pub(crate) fn new_key(pgp: &Pgp, email: &str) -> PrivateKey {
    pgp.generate_key().user_id(email, email).generate().unwrap()
}

pub(crate) struct Token {
    pub(crate) secret: String,
    pub(crate) sealed: String,
    pub(crate) signature: String,
}

pub(crate) fn token(pgp: &Pgp, user_key: &PrivateKey) -> Token {
    let secret = random_hex(32);
    let sealed = pgp
        .encrypt()
        .to(user_key.public_key())
        .armored_message(&secret)
        .unwrap();
    let signature = pgp
        .sign()
        .with_key(user_key.clone())
        .armored_detached(&secret)
        .unwrap();

    Token {
        secret,
        sealed,
        signature,
    }
}

pub(crate) fn user_key(id: &str, armored: &str, active: bool) -> Value {
    json!({
        "ID": id,
        "Version": 3,
        "PrivateKey": armored,
        "Primary": 1,
        "Active": u8::from(active)
    })
}

pub(crate) fn address_key(id: &str, armored: &str, token: Option<&Token>, active: bool) -> Value {
    let mut key = json!({
        "ID": id,
        "Version": 3,
        "Flags": 3,
        "PrivateKey": armored,
        "Primary": 1,
        "Active": u8::from(active)
    });

    if let Some(token) = token {
        key["Token"] = Value::String(token.sealed.clone());
        key["Signature"] = Value::String(token.signature.clone());
    }

    key
}

pub(crate) fn address(id: &str, email: &str, keys: Vec<Value>) -> Value {
    json!({
        "ID": id,
        "Email": email,
        "Send": 1,
        "Receive": 1,
        "Status": 1,
        "Type": 1,
        "Order": 1,
        "HasKeys": u8::from(!keys.is_empty()),
        "Keys": keys
    })
}

pub(crate) struct State {
    pub(crate) user_keys: Vec<Value>,
    pub(crate) addresses: Vec<Value>,
    pub(crate) salts: Vec<Value>,
    pub(crate) srp: Arc<FakeSrp>,
    pub(crate) asks: Vec<Request>,
    pub(crate) role: i32,
}

#[derive(Clone)]
pub(crate) struct Server {
    state: Arc<Mutex<State>>,
}

impl Server {
    pub(crate) fn new(user_keys: Vec<Value>, addresses: Vec<Value>) -> Server {
        Server {
            state: Arc::new(Mutex::new(State {
                user_keys,
                addresses,
                salts: vec![json!({"ID": "user-key", "KeySalt": SALT})],
                srp: Arc::new(FakeSrp::with_password(PASSWORD)),
                asks: Vec::new(),
                role: 0,
            })),
        }
    }

    pub(crate) fn with<T>(&self, change: impl FnOnce(&mut State) -> T) -> T {
        change(&mut self.state.lock().unwrap())
    }

    pub(crate) fn last_body(&self, method: &str, path: &str) -> Option<Value> {
        self.with(|state| {
            state
                .asks
                .iter()
                .rev()
                .find(|request| request.route() == (method, path))
                .map(Request::json)
        })
    }

    pub(crate) fn request_paths(&self) -> Vec<(String, String)> {
        self.with(|state| {
            state
                .asks
                .iter()
                .map(|request| (request.method.clone(), request.path.clone()))
                .collect()
        })
    }

    pub(crate) fn prove(&self, sent: &Value) -> String {
        let srp = self.with(|state| state.srp.clone());

        srp.prove(
            sent["ClientEphemeral"].as_str().unwrap_or_default(),
            sent["ClientProof"].as_str().unwrap_or_default(),
        )
        .unwrap()
    }

    pub(crate) async fn account<H>(&self, routes: H) -> Account
    where
        H: Fn(&Server, &Request) -> Option<ResponseFuture> + Send + Sync + 'static,
    {
        let server = self.clone();

        account(move |request: Request| {
            server.with(|state| state.asks.push(request.clone()));

            routes(&server, &request)
                .or_else(|| server.standard(&request))
                .unwrap_or_else(not_found)
        })
        .await
    }

    fn standard(&self, request: &Request) -> Option<ResponseFuture> {
        let body = match request.route() {
            ("POST", "/auth/v4/sessions") => return Some(crate::support::unauth_session()),
            ("POST", "/auth/v4/info") => {
                let srp = self.with(|state| state.srp.clone());
                json!({
                    "Code": 1000,
                    "Modulus": SIGNED_MODULUS,
                    "ServerEphemeral": srp.challenge(),
                    "Version": 4,
                    "Salt": srp.salt(),
                    "SRPSession": "srp-session"
                })
            }
            ("GET", "/auth/v4/modulus") => {
                json!({"Code": 1000, "Modulus": SIGNED_MODULUS, "ModulusID": "modulus-id"})
            }
            ("GET", "/core/v4/users") => self.with(|state| {
                json!({
                    "Code": 1000,
                    "User": {
                        "ID": "user-value",
                        "Name": "tester",
                        "Role": state.role,
                        "Keys": state.user_keys
                    }
                })
            }),
            ("GET", "/core/v4/addresses") => self.with(|state| {
                json!({
                    "Code": 1000,
                    "Total": state.addresses.len(),
                    "Addresses": state.addresses
                })
            }),
            ("GET", "/core/v4/keys/salts") => {
                self.with(|state| json!({"Code": 1000, "KeySalts": state.salts}))
            }
            ("GET", path) if path.starts_with("/core/v4/addresses/") => {
                let id = path.trim_start_matches("/core/v4/addresses/");
                let found = self.with(|state| {
                    state
                        .addresses
                        .iter()
                        .find(|address| address["ID"] == id)
                        .cloned()
                })?;
                json!({"Code": 1000, "Address": found})
            }
            _ => return None,
        };

        Some(response(200, body.to_string()))
    }
}

pub(crate) fn ok() -> ResponseFuture {
    response(200, json!({"Code": 1000}).to_string())
}

pub(crate) struct Migrated {
    pub(crate) server: Server,
    pub(crate) user_key: PrivateKey,
    pub(crate) address_key: PrivateKey,
    pub(crate) token: Token,
}

pub(crate) fn migrated(pgp: &Pgp) -> Migrated {
    let user = new_key(pgp, "not_for_email_use@domain.tld");
    let original = new_key(pgp, EMAIL);
    let token = token(pgp, &user);
    let server = Server::new(
        vec![user_key(
            "user-key",
            &lock_key(&user, passphrase(PASSWORD, SALT)),
            true,
        )],
        vec![address(
            "address-value",
            EMAIL,
            vec![address_key(
                "address-key",
                &lock_key(&original, &token.secret),
                Some(&token),
                true,
            )],
        )],
    );

    Migrated {
        server,
        user_key: user,
        address_key: original,
        token,
    }
}
