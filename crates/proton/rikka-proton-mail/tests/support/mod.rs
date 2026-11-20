use std::sync::Arc;

use rikka_proton_account::Account;
use rikka_proton_mail::MailClient;
use rikka_proton_pgp::PrivateKey;
use rikka_test_proton::{Owner, PASSPHRASE, Request, ResponseFuture, json_response, signed_in_api};
use serde_json::{Value, json};

pub(crate) struct SignedIn {
    pub(crate) client: MailClient,
    pub(crate) address_key: PrivateKey,
}

pub(crate) async fn signed_in<H>(handler: H) -> SignedIn
where
    H: Fn(Request) -> ResponseFuture + Send + Sync + 'static,
{
    signed_in_with(json!({}), handler).await
}

pub(crate) async fn signed_in_with<H>(settings: Value, handler: H) -> SignedIn
where
    H: Fn(Request) -> ResponseFuture + Send + Sync + 'static,
{
    let owner = Owner::new();
    let address_key = owner.address.clone();
    let settings = json!({"Code": 1000, "MailSettings": settings});
    let account = Account::new(
        signed_in_api(move |request: Request| match request.route() {
            ("GET", "/mail/v4/settings") => json_response(200, &settings),
            _ => owner.routes(&request).unwrap_or_else(|| handler(request)),
        })
        .await,
    );
    let keys = account.unlock_with_passphrase(PASSPHRASE).await.unwrap();

    SignedIn {
        client: MailClient::for_account(&account, &Arc::new(keys)),
        address_key,
    }
}
