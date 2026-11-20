use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rikka_proton_cache::Cache;
use rikka_proton_mail::{Event, LabelId, LabelType, MailClient, MessageId};
use rikka_proton_pgp::{Pgp, SymmetricKey};
use serde_json::json;

use crate::support::signed_in;
use rikka_test_proton::{Request, json_response, new_key, not_found};

fn message(id: &str, labels: &[&str]) -> serde_json::Value {
    json!({
        "ID": id,
        "ConversationID": format!("conversation-{id}"),
        "AddressID": rikka_test_proton::ADDRESS_ID,
        "Subject": format!("about {id}"),
        "Sender": {"Address": "friend@proton.test", "Name": "Friend"},
        "LabelIDs": labels,
        "Time": 1_700_000_000,
        "Unread": 1
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn listings_cached_and_updated_by_events() {
    let signed = signed_in(move |request: Request| match request.route() {
        ("GET", "/mail/v4/messages") => json_response(
            200,
            &json!({"Code": 1000, "Total": 2, "Messages": [message("one", &["0"]), message("two", &["0"])]}),
        ),
        ("GET", "/core/v4/labels") if request.query.contains("Type=3") => json_response(
            200,
            &json!({"Code": 1000, "Labels": [{"ID": "folder", "Name": "Work", "Type": 3, "Order": 1}]}),
        ),
        ("GET", "/core/v4/labels") => json_response(200, &json!({"Code": 1000, "Labels": []})),
        ("GET", "/mail/v4/messages/count") => json_response(
            200,
            &json!({"Code": 1000, "Counts": [{"LabelID": "0", "Total": 2, "Unread": 2}]}),
        ),
        _ => not_found(),
    })
    .await;
    let cache = Cache::in_memory(SymmetricKey::generate()).unwrap();
    let client = signed.client.with_cache(cache.clone());
    let inbox = LabelId::from("0");
    client.message_counts().await.unwrap();

    assert!(client.messages().label(&inbox).cached().is_none());
    assert!(client.labels().cached().is_none());

    client.messages().label(&inbox).list().await.unwrap();
    client.labels().list().await.unwrap();
    cache.flush().unwrap();

    let recalled = client.messages().label(&inbox).cached().unwrap();
    assert_eq!(recalled.len(), 2);
    assert!(
        client
            .messages()
            .label(&LabelId::from("5"))
            .cached()
            .is_none()
    );
    let folders = client.labels().kind(LabelType::Folder).cached().unwrap();
    assert_eq!(folders.len(), 1);

    let event: Event = serde_json::from_value(json!({
        "EventID": "next",
        "Messages": [
            {"ID": "one", "Action": 0},
            {"ID": "two", "Action": 3, "Message": message("two", &["6"])}
        ],
        "Labels": [{"ID": "folder", "Action": 0}],
        "MessageCounts": [{"LabelID": "0", "Total": 0, "Unread": 0}, {"LabelID": "6", "Total": 1, "Unread": 1}]
    }))
    .unwrap();
    client.apply_event(&event);

    assert!(client.messages().label(&inbox).cached().unwrap().is_empty());
    let counts = client.cached_message_counts().unwrap();
    assert_eq!(
        counts
            .iter()
            .map(|count| (count.label_id().as_str(), count.total(), count.unread()))
            .collect::<Vec<_>>(),
        [("0", 0, 0), ("6", 1, 1)]
    );
    assert_eq!(
        client
            .cached_message(&MessageId::from("two"))
            .unwrap()
            .label_ids(),
        &[LabelId::from("6")]
    );
    assert!(
        client
            .labels()
            .kind(LabelType::Folder)
            .cached()
            .is_some_and(|folders| folders.is_empty())
    );

    let refresh: Event = serde_json::from_value(json!({"EventID": "again", "Refresh": 1})).unwrap();
    client.apply_event(&refresh);
    assert!(client.labels().cached().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn sender_keys_fetched_once_per_cache() {
    let pgp = Pgp::new();
    let friend = new_key(&pgp, "friend@proton.test");
    let armored = friend.public_key().export_armored().unwrap();
    let request = Arc::new(AtomicUsize::new(0));
    let counting = Arc::clone(&request);

    let signed = signed_in(move |request: Request| match request.route() {
        ("GET", "/core/v4/keys/all") => {
            counting.fetch_add(1, Ordering::SeqCst);
            json_response(
                200,
                &json!({"Code": 1000, "ProtonMX": true, "IsProton": 1,
                        "Address": {"Keys": [{"Flags": 3, "PublicKey": armored}]}}),
            )
        }
        _ => not_found(),
    })
    .await;
    let cache = Cache::in_memory(SymmetricKey::generate()).unwrap();
    let first = signed.client.clone().with_cache(cache.clone());
    let keys = first.sender_keys("Friend@proton.test").await.unwrap();
    assert_eq!(keys.len(), 1);
    cache.flush().unwrap();

    let second = MailClient::for_account(
        signed.client.account(),
        &Arc::new(signed.client.keys().clone()),
    )
    .with_cache(cache);
    let recalled = second.sender_keys("friend@proton.test").await.unwrap();
    assert_eq!(recalled[0].fingerprint(), friend.public_key().fingerprint());
    assert_eq!(request.load(Ordering::SeqCst), 1);
}
