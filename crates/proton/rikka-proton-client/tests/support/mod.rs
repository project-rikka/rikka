use std::path::Path;
use std::time::Duration;

use rikka_certificate_transparency::{LogListKey, LogListSource, LogLists};
use rikka_proton_api::{Environment, Session as ApiSession};
use rikka_proton_client::{Accounts, Proton, Session};
use rikka_proton_pgp::{Pgp, SymmetricKey};
use rikka_test_proton::{
    PASSPHRASE, Request, ResponseFuture, lock_key, new_key, response, success,
};
use serde_json::json;

pub(crate) struct TestAccount {
    pub(crate) id: &'static str,
    pub(crate) email: &'static str,
    user: String,
    addresses: String,
}

impl TestAccount {
    pub(crate) fn new(id: &'static str, email: &'static str) -> TestAccount {
        let pgp = Pgp::new();
        let user_key = lock_key(&new_key(&pgp, email), PASSPHRASE);
        let address_key = lock_key(&new_key(&pgp, email), PASSPHRASE);
        TestAccount {
            id,
            email,
            user: json!({
                "Code": 1000,
                "User": {
                    "ID": id,
                    "Name": id,
                    "Email": email,
                    "Keys": [{"ID": format!("{id}-user-key"), "Version": 3, "PrivateKey": user_key, "Primary": 1, "Active": 1}]
                }
            })
            .to_string(),
            addresses: json!({
                "Code": 1000,
                "Total": 1,
                "Addresses": [{
                    "ID": format!("{id}-address"),
                    "Email": email,
                    "Send": 1, "Receive": 1, "Status": 1, "Type": 1, "Order": 1, "HasKeys": 1,
                    "Keys": [{"ID": format!("{id}-address-key"), "Version": 3, "Flags": 3, "PrivateKey": address_key, "Primary": 1, "Active": 1}]
                }]
            })
            .to_string(),
        }
    }

    pub(crate) fn addresses_named(&self, name: &str) -> String {
        let mut addresses: serde_json::Value = serde_json::from_str(&self.addresses).unwrap();
        addresses["Addresses"][0]["DisplayName"] = json!(name);
        addresses.to_string()
    }

    pub(crate) fn token(&self) -> String {
        format!("{}-access", self.id)
    }

    pub(crate) fn respond(&self, request: &Request) -> Option<ResponseFuture> {
        if request.bearer() != self.token() {
            return None;
        }
        match request.route() {
            ("GET", "/core/v4/users") => Some(response(200, self.user.clone())),
            ("GET", "/core/v4/addresses") => Some(response(200, self.addresses.clone())),
            ("DELETE", "/auth/v4") => Some(success()),
            _ => None,
        }
    }
}

pub(crate) async fn accounts(address: &str, root: &Path, key: SymmetricKey) -> Accounts {
    builder(address).accounts(root, key).await.unwrap()
}

pub(crate) fn builder(address: &str) -> rikka_proton_client::ProtonBuilder {
    let source = LogListSource::new(
        &format!("{address}/ct/log_list.json"),
        &format!("{address}/ct/log_list.sig"),
        LogListKey::chrome(),
    )
    .unwrap();
    Proton::builder()
        .app_version("linux-mail@1.0.0")
        .environment(Environment::custom(address).unwrap())
        .timeout(Duration::from_secs(5))
        .log_lists(LogLists::builder().source(source).build())
}

pub(crate) async fn signed_in(accounts: &Accounts, person: &TestAccount) -> Session {
    let proton = accounts.signing_in().unwrap();
    proton
        .account()
        .api()
        .set_session(ApiSession::new(
            format!("{}-uid", person.id),
            person.token(),
            format!("{}-refresh", person.id),
        ))
        .unwrap();
    let session = proton.unlock_with_passphrase(PASSPHRASE).await.unwrap();
    accounts.adopt(session).await.unwrap()
}
