use std::sync::Arc;

use rikka_proton_account::UserId;
use rikka_proton_client::{AccountState, Error};
use rikka_proton_pgp::SymmetricKey;

use crate::support::{TestAccount, accounts, builder, signed_in};
use rikka_test_proton::{PASSPHRASE, Record, Request, not_found, serve};

#[tokio::test(flavor = "multi_thread")]
async fn accounts_switch_disable_remove() {
    let alice = Arc::new(TestAccount::new("alice", "alice@proton.test"));
    let bob = Arc::new(TestAccount::new("bob", "bob@proton.test"));
    let record = Record::default();
    let address = {
        let (alice, bob, record) = (Arc::clone(&alice), Arc::clone(&bob), record.clone());
        serve(move |request: Request| {
            record.push(&request);
            alice
                .respond(&request)
                .or_else(|| bob.respond(&request))
                .unwrap_or_else(not_found)
        })
        .await
    };
    let root = tempfile::tempdir().unwrap();
    let key = SymmetricKey::generate();
    let (alice_id, bob_id) = (UserId::from(alice.id), UserId::from(bob.id));

    let book = accounts(&address, root.path(), key.clone()).await;
    assert!(book.list().is_empty());
    let first = signed_in(&book, &alice).await;
    signed_in(&book, &bob).await;
    assert_eq!(first.keys().user().id(), &alice_id);
    assert_eq!(book.primary(), Some(alice_id.clone()));
    book.set_primary(&bob_id).await.unwrap();
    drop(book);

    let reopened = accounts(&address, root.path(), key.clone()).await;
    let listed = reopened.list();
    assert_eq!(listed.len(), 2);
    assert!(
        listed
            .iter()
            .all(|info| info.state() == AccountState::Ready)
    );
    let primary = listed.iter().find(|info| info.is_primary()).unwrap();
    assert_eq!(primary.user(), &bob_id);
    assert_eq!(primary.email(), Some(bob.email));

    let resumed = reopened.resume(&alice_id, PASSPHRASE).await.unwrap();
    assert_eq!(
        resumed
            .proton()
            .account()
            .api()
            .session()
            .unwrap()
            .access_token(),
        alice.token()
    );
    assert!(resumed.proton().account().cache().is_some());

    reopened.disable(&alice_id).await.unwrap();
    assert_eq!(record.count("DELETE", "/auth/v4"), 1);
    assert_eq!(reopened.state(&alice_id), AccountState::Disabled);
    assert!(matches!(
        reopened.resume(&alice_id, PASSPHRASE).await,
        Err(Error::NoSuchAccount(_))
    ));

    reopened.remove(&bob_id).await.unwrap();
    assert_eq!(reopened.primary(), None);
    let left = reopened.list();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].user(), &alice_id);
    assert_eq!(left[0].state(), AccountState::Disabled);
    let folders = std::fs::read_dir(root.path())
        .unwrap()
        .filter(|entry| entry.as_ref().is_ok_and(|entry| entry.path().is_dir()))
        .count();
    assert_eq!(folders, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_account_list_with_wrong_key() {
    let alice = Arc::new(TestAccount::new("alice", "alice@proton.test"));
    let address = {
        let alice = Arc::clone(&alice);
        serve(move |request: Request| alice.respond(&request).unwrap_or_else(not_found)).await
    };
    let root = tempfile::tempdir().unwrap();
    let book = accounts(&address, root.path(), SymmetricKey::generate()).await;
    signed_in(&book, &alice).await;

    let refused = builder(&address)
        .accounts(root.path(), SymmetricKey::generate())
        .await
        .unwrap_err();
    assert!(matches!(refused, Error::InvalidAccountList(_)), "{refused}");
}
