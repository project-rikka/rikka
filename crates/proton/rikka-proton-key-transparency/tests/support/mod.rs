pub(crate) mod tree;

use rikka_proton_account::{Account, AccountKeys, Profile};
use rikka_proton_api::Api;
use rikka_proton_key_transparency::{KeyTransparency, LogLists, Realm};
use rikka_proton_pgp::{Clock, Pgp, PrivateKey, SecretBytes};
use rikka_test_certificate_transparency::serving;
use rikka_test_proton::{Request, ResponseFuture, builder, response, serve};
use serde_json::json;

pub(crate) const NOW: u64 = 1_789_948_800;
const LOG_LIST_PUBLISHED: &str = "2026-09-20T00:00:00Z";
pub(crate) const PASSPHRASE: &[u8] = b"the mailbox passphrase";

pub(crate) async fn api<H>(handler: H) -> Api
where
    H: Fn(Request) -> ResponseFuture + Send + Sync + 'static,
{
    builder(&serve(handler).await).retries(0).build().unwrap()
}

pub(crate) fn absent() -> ResponseFuture {
    response(422, r#"{"Code":2501,"Error":"Not found"}"#)
}

pub(crate) fn pgp() -> Pgp {
    Pgp::with_clock(Clock::fixed(NOW))
}

pub(crate) fn log_lists() -> LogLists {
    serving(LOG_LIST_PUBLISHED)
}

pub(crate) fn transparency(api: Api) -> KeyTransparency {
    KeyTransparency::builder(api)
        .pgp(pgp())
        .realm(Realm::production())
        .log_lists(log_lists())
        .build()
}

pub(crate) fn new_key(email: &str) -> PrivateKey {
    pgp()
        .generate_key()
        .user_id("tester", email)
        .generate()
        .unwrap()
}

pub(crate) async fn account_keys(api: Api, user_key: &PrivateKey) -> AccountKeys {
    let locked = user_key.lock(PASSPHRASE).unwrap().export_armored().unwrap();
    let profile: Profile = serde_json::from_value(json!({
        "User": {
            "ID": "user",
            "Keys": [{"ID": "user-key", "Version": 3, "PrivateKey": locked, "Primary": 1, "Active": 1}]
        },
        "Addresses": [{
            "ID": "address",
            "Email": "self@proton.test",
            "Status": 1,
            "Type": 1,
            "Keys": []
        }]
    }))
    .unwrap();

    Account::with_pgp(api, pgp())
        .unlock_profile(profile, Some(SecretBytes::new(PASSPHRASE)))
        .await
        .unwrap()
}
