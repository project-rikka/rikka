use jiff::Timestamp;
use rikka_certificate_transparency::{Error, ListUpdate, LogLists};
use rikka_test_certificate_transparency::{LogListServer, Signer, log_list_at};

use crate::support::at;

const PUBLISHED: &str = "2030-01-01T00:00:00Z";
const NOW: &str = "2030-01-02T00:00:00Z";

fn timestamp_of(lists: &LogLists) -> Option<Timestamp> {
    lists.current().map(|list| list.timestamp())
}

#[tokio::test]
async fn refresh_adopts_and_caches_signed_list() {
    let server = LogListServer::start();
    let signer = Signer::new();
    let list = log_list_at(PUBLISHED);
    server.publish(&list, &signer.sign(&list));
    let cache = tempfile::tempdir().unwrap();
    let open = || {
        LogLists::builder()
            .source(server.source(signer.key()))
            .cache_dir(cache.path())
            .build()
    };

    let lists = open();
    assert_eq!(timestamp_of(&lists), None);
    assert_eq!(
        lists.refresh_if_due(at(NOW)).await.unwrap(),
        ListUpdate::Updated
    );
    assert_eq!(timestamp_of(&lists), Some(at(PUBLISHED)));

    assert_eq!(timestamp_of(&open()), Some(at(PUBLISHED)));
}

#[tokio::test]
async fn refresh_keeps_current_list_against_older_or_forged_lists() {
    let server = LogListServer::start();
    let signer = Signer::new();
    let list = log_list_at(PUBLISHED);
    server.publish(&list, &signer.sign(&list));
    let lists = LogLists::builder()
        .source(server.source(signer.key()))
        .build();
    lists.refresh_if_due(at(NOW)).await.unwrap();

    let older = log_list_at("2029-01-01T00:00:00Z");
    let forged = log_list_at("2030-06-01T00:00:00Z");
    let cases = [
        (
            "2030-01-04T00:00:00Z",
            older.clone(),
            signer.sign(&older),
            Some(ListUpdate::Unchanged),
        ),
        (
            "2030-01-06T00:00:00Z",
            forged.clone(),
            Signer::new().sign(&forged),
            None,
        ),
    ];
    for (now, list, signature, expected) in cases {
        server.publish(&list, &signature);
        let outcome = lists.refresh_if_due(at(now)).await;
        match expected {
            Some(update) => assert_eq!(outcome.unwrap(), update),
            None => assert!(matches!(outcome, Err(Error::Signature))),
        }
        assert_eq!(timestamp_of(&lists), Some(at(PUBLISHED)));
    }
}

#[tokio::test]
async fn missing_list_is_fetched_before_use() {
    let server = LogListServer::start();
    let signer = Signer::new();
    let list = log_list_at(PUBLISHED);
    server.publish(&list, &signer.sign(&list));
    let lists = LogLists::builder()
        .source(server.source(signer.key()))
        .build();

    let fresh = lists.fresh(at(NOW)).await.unwrap();
    assert_eq!(fresh.timestamp(), at(PUBLISHED));
}

#[tokio::test]
async fn unreachable_source_leaves_list_unusable_without_retrying_at_once() {
    let signer = Signer::new();
    let cache = tempfile::tempdir().unwrap();
    let publisher = LogListServer::start();
    let list = log_list_at(PUBLISHED);
    publisher.publish(&list, &signer.sign(&list));
    LogLists::builder()
        .source(publisher.source(signer.key()))
        .cache_dir(cache.path())
        .build()
        .refresh_if_due(at(NOW))
        .await
        .unwrap();
    let later = "2031-01-01T00:00:00Z";

    for cached in [false, true] {
        let server = LogListServer::start();
        let mut builder = LogLists::builder().source(server.source(signer.key()));
        if cached {
            builder = builder.cache_dir(cache.path());
        }
        let lists = builder.build();
        let unusable = |outcome: &Result<_, Error>| match outcome {
            Err(Error::Expired(_)) => cached,
            Err(Error::NotFetched) => !cached,
            _ => false,
        };

        assert!(unusable(&lists.fresh(at(later)).await));
        let requests = server.requests();
        assert!(requests > 0);
        assert!(unusable(&lists.fresh(at(later)).await));
        assert_eq!(server.requests(), requests);
    }
}
