use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rikka_proton_cache::Cache;
use rikka_proton_pass::{EncryptionTag, ItemId, PassClient};
use rikka_proton_pgp::SymmetricKey;
use serde_json::json;

use crate::support::{
    ITEM, KeyMaterial, SHARE, login, revision_json, seal_base64, seeded, share_json,
};
use rikka_test_proton::{Request, response};

const ADDED: &str = "item-added";

#[tokio::test(flavor = "multi_thread")]
async fn items_sync_from_cache_via_events() {
    let seeds = KeyMaterial::new();
    let offline = Arc::new(AtomicBool::new(false));
    let refusing = Arc::clone(&offline);
    let added = revision_json(
        ADDED,
        1,
        &seal_base64(
            &seeds.item_key,
            login("Added", "hunter3").encode(),
            &EncryptionTag::ItemContent,
        ),
        &seal_base64(
            &seeds.share_key,
            seeds.item_key.as_bytes(),
            &EncryptionTag::ItemKey,
        ),
        None,
    );
    let listed = json!({
        "Code": 1000,
        "Items": {"Total": 1, "LastToken": null, "RevisionsData": [seeds.item(3)]}
    })
    .to_string();
    let shares = json!({"Code": 1000, "Shares": [share_json(SHARE, &seeds.share_key)]}).to_string();
    let events = json!({
        "Code": 1000,
        "Events": {
            "UpdatedItems": [added],
            "DeletedItemIDs": [ITEM],
            "LatestEventID": "event-2",
            "EventsPending": false
        }
    })
    .to_string();

    let fixture = seeded(seeds, move |request: &Request| {
        if refusing.load(Ordering::SeqCst) {
            return Some(response(503, r#"{"Code":503}"#));
        }
        let listing = format!("/pass/v1/share/{SHARE}/item");
        let latest = format!("/pass/v1/share/{SHARE}/event");
        let since = format!("/pass/v1/share/{SHARE}/event/event-1");
        match request.route() {
            ("GET", "/pass/v1/share") => Some(response(200, shares.clone())),
            ("GET", path) if path == listing => Some(response(200, listed.clone())),
            ("GET", path) if path == latest => {
                Some(response(200, r#"{"Code":1000,"EventID":"event-1"}"#))
            }
            ("GET", path) if path == since => Some(response(200, events.clone())),
            _ => None,
        }
    })
    .await;

    let cache = Cache::in_memory(SymmetricKey::generate()).unwrap();
    let client: PassClient = fixture.client.clone().with_cache(cache.clone());
    let share = fixture.share();

    let first = client.sync_items(&share).await.unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].id(), &ItemId::from(ITEM));
    client.shares().await.unwrap();
    cache.flush().unwrap();

    let second = client.sync_items(&share).await.unwrap();
    assert_eq!(
        second
            .iter()
            .map(|item| item.id().clone())
            .collect::<Vec<_>>(),
        vec![ItemId::from(ADDED)]
    );
    cache.flush().unwrap();

    offline.store(true, Ordering::SeqCst);
    let offline = PassClient::with_pgp(client.api().clone(), *client.pgp(), fixture.user_keys())
        .with_cache(cache);
    let shares = offline.cached_shares().await.unwrap().unwrap();
    assert_eq!(shares.len(), 1);
    assert!(shares[0].vault().is_some());
    let items = offline.cached_items(&share).await.unwrap().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name(), "Added");
}
