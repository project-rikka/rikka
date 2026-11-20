use std::sync::Arc;

use rikka_proton_account::Account;
use rikka_proton_contacts::ContactsClient;
use rikka_proton_pgp::PrivateKey;
use rikka_test_proton::{Owner, PASSPHRASE, Request, ResponseFuture, signed_in_api};

pub(crate) struct SignedIn {
    pub(crate) account: Account,
    pub(crate) client: ContactsClient,
    pub(crate) user_key: PrivateKey,
}

pub(crate) async fn signed_in<H>(handler: H) -> SignedIn
where
    H: Fn(Request) -> ResponseFuture + Send + Sync + 'static,
{
    let owner = Owner::new();
    let user_key = owner.user.clone();
    let account = Account::new(
        signed_in_api(move |request: Request| {
            owner.routes(&request).unwrap_or_else(|| handler(request))
        })
        .await,
    );
    let keys = account.unlock_with_passphrase(PASSPHRASE).await.unwrap();

    SignedIn {
        client: ContactsClient::for_account(&account, &Arc::new(keys)),
        account,
        user_key,
    }
}
