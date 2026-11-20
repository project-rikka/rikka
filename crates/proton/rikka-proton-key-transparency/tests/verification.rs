use std::sync::{Arc, Mutex};

use rikka_proton_account::{
    AddressKeyFlags, KeyDifference, KeyList, PublishedKeys, SignedKeyListItem,
};
use rikka_proton_key_transparency::{
    Error, Failure, KeyTransparency, LogListError, LogLists, Pending, Realm, Unverifiable, Verdict,
};
use rikka_proton_pgp::{Clock, Pgp};
use rikka_test_certificate_transparency::{LogListServer, Signer, log_list_at};
use serde_json::{Value, json};

use crate::fixtures::EPOCH_1509;
use crate::support::{absent, api, log_lists, new_key, pgp, transparency};
use rikka_test_proton::response;

fn published(email: &str, signed_key_list: Value) -> (PublishedKeys, String) {
    let key = new_key(email).public_key();
    let data = KeyList::new([SignedKeyListItem::new(
        &key,
        true,
        AddressKeyFlags::from_bits(3),
    )])
    .to_data();
    let mut list = signed_key_list;
    if list.is_object() && list.get("Data").is_none() {
        list["Data"] = Value::from(data.clone());
    }
    let keys = serde_json::from_value(json!({
        "Keys": [{"PublicKey": key.export_armored().unwrap(), "Flags": 3, "Primary": 1}],
        "SignedKeyList": list
    }))
    .unwrap();
    (keys, data)
}

#[tokio::test]
async fn excluded_domains_are_unverifiable() {
    let api = api(|request| match request.route() {
        ("GET", "/core/v4/keys/all") => response(
            200,
            json!({"Code": 1000, "Address": {"Keys": []}, "ProtonMX": false}).to_string(),
        ),
        _ => absent(),
    })
    .await;

    let found = transparency(api)
        .lookup("someone@Hotmail.com")
        .skipping_excluded_domains()
        .send()
        .await
        .unwrap();

    let excluded = Verdict::Unverifiable(Unverifiable::ExcludedDomain);
    assert_eq!(found.verdicts().address(), &excluded);
    assert_eq!(found.verdicts().catch_all(), Some(&excluded));
}

#[tokio::test]
async fn pending_key_lists_kept_for_audit() {
    let api = api(|_| absent()).await;
    let persisted = Arc::new(Mutex::new(Vec::new()));
    let stored = persisted.clone();
    let transparency = KeyTransparency::builder(api)
        .pgp(pgp())
        .log_lists(log_lists())
        .persist(move |state| stored.lock().unwrap().push(state.clone()))
        .build();

    let (keys, data) = published(
        "alice@proton.test",
        json!({"Signature": "signature", "Revision": 2, "MinEpochID": null, "ExpectedMinEpochID": 77}),
    );
    let verdicts = transparency
        .check("alice@proton.test")
        .address(&keys)
        .verify()
        .await
        .unwrap();
    assert_eq!(verdicts.address(), &Verdict::Pending(Pending::Keys));
    assert!(verdicts.keys_changed_recently());

    let state = transparency.state();
    let [pending] = state.pending() else {
        panic!("expected one pending key list");
    };
    assert_eq!(pending.email(), "alice@proton.test");
    assert_eq!(pending.content(), data);
    assert_eq!(pending.revision(), 2);
    assert_eq!(pending.expected_min_epoch_id(), 77);
    assert_eq!(persisted.lock().unwrap().last(), Some(&state));
}

#[tokio::test]
async fn inconsistent_key_lists_fail_early() {
    let api = api(|_| absent()).await;
    let transparency = transparency(api);

    let (keys, _) = published(
        "bob@proton.test",
        json!({"Signature": "signature", "Revision": 0, "MinEpochID": 3}),
    );
    let verdict = transparency
        .verify_keys(
            "bob@proton.test",
            &keys.signed_key_list_items(),
            keys.signed_key_list(),
        )
        .await
        .unwrap();
    assert_eq!(verdict, Verdict::Failed(Failure::RevisionZero));

    let (keys, _) = published(
        "bob@proton.test",
        json!({"Signature": "signature", "Revision": 4, "MinEpochID": 3}),
    );
    let other = new_key("mallory@proton.test").public_key();
    let verdict = transparency
        .verify_keys(
            "bob@proton.test",
            &[SignedKeyListItem::new(
                &other,
                true,
                AddressKeyFlags::from_bits(3),
            )],
            keys.signed_key_list(),
        )
        .await
        .unwrap();
    assert!(matches!(
        verdict,
        Verdict::Failed(Failure::KeyMismatch {
            difference: KeyDifference::Fingerprint,
            ..
        })
    ));
}

#[tokio::test]
async fn invalid_epoch_fails_keys() {
    let epoch: Value = serde_json::from_str(EPOCH_1509).unwrap();
    let api = api(move |request| match request.route() {
        ("GET", "/kt/v1/epochs") => response(
            200,
            json!({"Code": 1000, "Total": 1, "Epochs": [epoch.clone()]}).to_string(),
        ),
        _ => absent(),
    })
    .await;

    let (keys, _) = published(
        "carol@proton.test",
        json!({"Signature": "signature", "Revision": 4, "MinEpochID": 1500}),
    );
    let verdict = transparency(api)
        .verify_keys(
            "carol@proton.test",
            &keys.signed_key_list_items(),
            keys.signed_key_list(),
        )
        .await
        .unwrap();
    assert!(matches!(
        verdict,
        Verdict::Failed(Failure::CertificateChain(_))
    ));
}

#[tokio::test]
async fn unfetchable_log_list_disables_kt() {
    let signer = Signer::new();
    for published in [false, true] {
        let server = LogListServer::start();
        if published {
            let list = log_list_at("2033-05-18T00:00:00Z");
            server.publish(&list, &signer.sign(&list));
        }
        let transparency = KeyTransparency::builder(api(|_| absent()).await)
            .pgp(Pgp::with_clock(Clock::fixed(2_000_000_000)))
            .realm(Realm::production())
            .log_lists(
                LogLists::builder()
                    .source(server.source(signer.key()))
                    .build(),
            )
            .build();

        assert_eq!(transparency.is_available().await, published);
        if !published {
            let response = transparency
                .verify_keys("dave@proton.test", &[], None)
                .await;
            assert!(matches!(
                response,
                Err(Error::LogList(LogListError::NotFetched))
            ));
        }
    }
}
