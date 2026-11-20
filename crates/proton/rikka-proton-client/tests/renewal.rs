use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rikka_proton_client::Renewal;
use rikka_proton_pgp::SymmetricKey;
use rikka_test_proton::{Request, not_found, response, serve};

use crate::support::{TestAccount, accounts, signed_in};

#[tokio::test(flavor = "multi_thread")]
async fn profile_change_keeps_keys_and_renames_address() {
    let alice = Arc::new(TestAccount::new("alice", "alice@proton.test"));
    let renamed = Arc::new(AtomicBool::new(false));
    let address = {
        let (alice, renamed) = (Arc::clone(&alice), Arc::clone(&renamed));
        serve(move |request: Request| {
            if renamed.load(Ordering::SeqCst) && request.route() == ("GET", "/core/v4/addresses") {
                return response(200, alice.addresses_named("Alice Renamed"));
            }
            alice.respond(&request).unwrap_or_else(not_found)
        })
        .await
    };
    let root = tempfile::tempdir().unwrap();
    let accounts = accounts(&address, root.path(), SymmetricKey::generate()).await;
    let session = signed_in(&accounts, &alice).await;

    renamed.store(true, Ordering::SeqCst);
    let Renewal::Updated(renewed) = session.renew().await.unwrap() else {
        panic!("a display name change keeps the keys");
    };

    assert_eq!(
        renewed.keys().addresses()[0].display_name(),
        Some("Alice Renamed")
    );
}
