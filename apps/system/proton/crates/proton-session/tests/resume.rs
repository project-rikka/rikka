use proton_session::{AccountStore, Renewed, Resumed};
use rikka_test_keyring::TestKeyring;

use crate::support::{Account, PASSWORD};

#[tokio::test(flavor = "multi_thread")]
async fn cached_session_resumes_offline() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let store = AccountStore::open_in(dir.path()).expect("failed to open store");
    let account = Account::new();
    account
        .access(&store)
        .await
        .unlock(PASSWORD)
        .await
        .expect("failed to unlock account");
    let requests = account.requests();

    let again = AccountStore::open_in(dir.path()).expect("failed to reopen store");
    let resumed = account
        .access(&again)
        .await
        .resume()
        .await
        .expect("failed to resume");

    let Resumed::Cached(session) = resumed else {
        panic!("cached session did not resume");
    };
    assert_eq!(account.requests(), requests);
    assert_eq!(session.keys().user().name(), Some("tester"));
}

#[tokio::test(flavor = "multi_thread")]
async fn renewing_signed_out_account_forgets_it() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let store = AccountStore::open_in(dir.path()).expect("failed to open store");
    let account = Account::new();
    let access = account.access(&store).await;
    let session = access
        .unlock(PASSWORD)
        .await
        .expect("failed to unlock account");
    account.sign_out();

    let renewed = access.renew(&session).await.expect("failed to renew");

    assert!(matches!(renewed, Renewed::SignedOut));
    assert!(store.saved_account().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn save_after_forget_leaves_nothing() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let store = AccountStore::open_in(dir.path()).expect("failed to open store");
    let account = Account::new();
    let access = account.access(&store).await;
    let session = access
        .unlock(PASSWORD)
        .await
        .expect("failed to unlock account");

    let keeping = access.keep_key(&session);
    access.forget().await;
    keeping.await;

    assert!(store.saved_account().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn renewal_after_sign_out_saves_nothing() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let store = AccountStore::open_in(dir.path()).expect("failed to open store");
    let account = Account::new();
    let access = account.access(&store).await;
    let session = access
        .unlock(PASSWORD)
        .await
        .expect("failed to unlock account");
    account.change_description();

    let renewing = access.renew(&session);
    access.forget().await;
    let renewed = renewing.await.expect("failed to renew");

    assert!(matches!(renewed, Renewed::ProfileChanged(_)));
    assert!(store.saved_account().is_none());
}
