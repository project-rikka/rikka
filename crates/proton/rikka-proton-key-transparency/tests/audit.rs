use rikka_proton_account::AddressId;
use rikka_proton_key_transparency::{AuditState, Error, Failure};
use rikka_proton_pgp::KeyRing;
use serde_json::json;

use crate::support::{NOW, absent, account_keys, api, new_key, pgp, transparency};
use rikka_test_proton::response;

#[tokio::test]
async fn failed_audits_back_off() {
    let user_key = new_key("self@proton.test");
    let api = api(|_| absent()).await;
    let keys = account_keys(api.clone(), &user_key).await;
    let transparency = transparency(api);

    for (attempts, delay) in [(1, 120), (2, 240), (3, 480)] {
        let report = transparency.audit(&keys).await.unwrap();
        let error = report.error().unwrap();
        assert_eq!(error.failed_attempts(), attempts);
        assert!(!error.retries_exhausted());
        assert_eq!(report.audited_at(), NOW);
        assert_eq!(report.next_audit_at(), NOW + delay);
        assert_eq!(transparency.state().last_audit(), Some(&report));
        assert!(!transparency.is_audit_due());
    }
}

#[tokio::test]
async fn published_key_lists_await_inclusion() {
    let api = api(|request| match request.route() {
        ("GET", "/core/v4/keys/signedkeylists") if request.query.contains("AfterRevision=4") => response(
            200,
            json!({
                "Code": 1000,
                "SignedKeyLists": [
                    {"Data": "[]", "Signature": "older", "Revision": 4, "MinEpochID": 80},
                    {"Data": "[\"published\"]", "Signature": "new", "Revision": 5, "ExpectedMinEpochID": 90}
                ]
            })
            .to_string(),
        ),
        _ => absent(),
    })
    .await;
    let transparency = transparency(api);
    let address = AddressId::from("address");

    transparency
        .record_published(&address, "self@proton.test", Some(4), "[\"published\"]")
        .await
        .unwrap();
    let state = transparency.state();
    let [pending] = state.pending() else {
        panic!("expected one pending key list");
    };
    assert_eq!(pending.address_id(), Some(&address));
    assert_eq!(pending.revision(), 5);
    assert_eq!(pending.expected_min_epoch_id(), 90);
    assert_eq!(pending.created_at(), NOW);

    let unknown = transparency
        .record_published(&address, "self@proton.test", Some(4), "[\"unknown\"]")
        .await;
    assert!(matches!(
        unknown,
        Err(Error::Failed(Failure::PublishedKeyListNotFound))
    ));
}

#[test]
fn sealed_state_requires_user_keys() {
    let state: AuditState = serde_json::from_value(json!({
        "pending": [{
            "created_at": NOW,
            "expected_min_epoch_id": 12,
            "email": "friend@proton.me",
            "data": "[]",
            "revision": 3,
            "catch_all": true
        }]
    }))
    .unwrap();
    let user_key = new_key("self@proton.test");
    let stranger = new_key("stranger@proton.test");

    let sealed = state.seal(&pgp(), &user_key).unwrap();
    let decrypted =
        AuditState::open(&sealed, &pgp(), &KeyRing::new().with_private(user_key)).unwrap();
    assert_eq!(decrypted, state);
    assert!(decrypted.pending()[0].is_catch_all());

    assert!(AuditState::open(&sealed, &pgp(), &KeyRing::new().with_private(stranger)).is_err());
}
