use jiff::Timestamp;
use rikka_proton_mail::{
    Error, Event, IndexStep, LabelId, MessageId, MessageMetadata, SearchIndex, SearchQuery,
};
use rikka_proton_pgp::{Pgp, SymmetricKey};
use serde_json::{Value, json};

use crate::support::signed_in;
use rikka_test_proton::{ADDRESS_ID, EMAIL, Request, json_response, not_found};

fn metadata(wire: Value) -> MessageMetadata {
    serde_json::from_value(wire).unwrap()
}

fn sample_index() -> SearchIndex {
    let mut index = SearchIndex::new();
    index.insert(
        metadata(json!({
            "ID": "m1", "AddressID": "a1", "Subject": "Résumé for the café job",
            "Sender": {"Address": "Zoe@Example.test", "Name": "Zoë Martin"},
            "ToList": [{"Address": "me@proton.test", "Name": "Me"}],
            "LabelIDs": ["0", "5"], "Time": 300, "NumAttachments": 1, "Unread": 1, "Flags": 1
        })),
        Some("Please find my résumé attached. I'm \u{2019}available\u{2019} on Monday."),
    );
    index.insert(
        metadata(json!({
            "ID": "m2", "AddressID": "a1", "Subject": "Invoice 42",
            "Sender": {"Address": "billing@shop.test", "Name": "Shop"},
            "ToList": [{"Address": "me@proton.test", "Name": "Me"}],
            "CCList": [{"Address": "accounting@proton.test", "Name": "Accounting"}],
            "LabelIDs": ["6", "5"], "Time": 200, "Flags": 1
        })),
        Some("Your invoice for the cafe order is due"),
    );
    index.insert(
        metadata(json!({
            "ID": "m3", "AddressID": "a2", "Subject": "Expired offer",
            "Sender": {"Address": "deals@shop.test", "Name": "Deals"},
            "LabelIDs": ["0", "5"], "Time": 100, "ExpirationTime": 1, "Flags": 1
        })),
        Some("cafe discount"),
    );
    index
}

fn search_ids(index: &SearchIndex, query: &SearchQuery) -> Vec<String> {
    index
        .search(query)
        .iter()
        .map(|metadata| metadata.id().to_string())
        .collect()
}

#[test]
fn queries_follow_proton_semantics() {
    let index = sample_index();
    let inbox = LabelId::from("0");

    let cases: Vec<(SearchQuery, Vec<&str>)> = vec![
        (SearchQuery::new().keywords("cafe"), vec!["m1", "m2"]),
        (SearchQuery::new().keywords("CAFÉ résumé"), vec!["m1"]),
        (
            SearchQuery::new().keywords("\"invoice for the\""),
            vec!["m2"],
        ),
        (
            SearchQuery::new().keywords("\u{201c}the cafe order\u{201d}"),
            vec!["m2"],
        ),
        (
            SearchQuery::new().keywords("\u{201c}for the cafe"),
            vec!["m1", "m2"],
        ),
        (SearchQuery::new().keywords("i'm 'available'"), vec!["m1"]),
        (SearchQuery::new().keywords("accounting"), vec!["m2"]),
        (SearchQuery::new().from("zoe@example"), vec!["m1"]),
        (SearchQuery::new().from("zoe martin"), vec!["m1"]),
        (SearchQuery::new().to("accounting@"), vec!["m2"]),
        (SearchQuery::new().subject("invoice"), vec!["m2"]),
        (
            SearchQuery::new().keywords("cafe").label(&inbox),
            vec!["m1"],
        ),
        (
            SearchQuery::new().begin(Timestamp::from_second(250).unwrap()),
            vec!["m1"],
        ),
        (
            SearchQuery::new().end(Timestamp::from_second(250).unwrap()),
            vec!["m2"],
        ),
        (SearchQuery::new().with_attachments(true), vec!["m1"]),
        (SearchQuery::new().unread(false), vec!["m2"]),
        (SearchQuery::new().oldest_first(), vec!["m2", "m1"]),
        (SearchQuery::new().limit(1), vec!["m1"]),
        (SearchQuery::new().keywords("nowhere"), vec![]),
    ];

    for (query, expected) in cases {
        assert_eq!(search_ids(&index, &query), expected, "{query:?}");
    }
}

#[test]
fn events_update_index() {
    let mut index = sample_index();
    let event: Event = serde_json::from_value(json!({
        "EventID": "e2",
        "Messages": [
            {"ID": "m1", "Action": 0},
            {"ID": "m4", "Action": 1, "Message": {
                "ID": "m4", "Subject": "Fresh", "LabelIDs": ["0"], "Time": 400, "Flags": 1
            }},
            {"ID": "m2", "Action": 2, "Message": {
                "ID": "m2", "Subject": "Invoice 42 draft", "LabelIDs": ["8"], "Time": 200, "Flags": 0
            }},
            {"ID": "m3", "Action": 3, "Message": {
                "ID": "m3", "Subject": "Expired offer", "LabelIDs": ["6"], "Time": 100, "Flags": 1
            }}
        ]
    }))
    .unwrap();

    let matches = index.apply(&event);

    assert_eq!(matches, vec![MessageId::from("m4"), MessageId::from("m2")]);
    assert!(!index.contains(&MessageId::from("m1")));
    assert!(!index.has_content(&MessageId::from("m4")));
    assert!(!index.has_content(&MessageId::from("m2")));
    assert!(index.has_content(&MessageId::from("m3")));
    assert_eq!(
        search_ids(&index, &SearchQuery::new().subject("draft")),
        vec!["m2"]
    );
}

#[test]
fn sealed_index_requires_key() {
    let index = sample_index();
    let key = SymmetricKey::generate();
    let blob = index.seal(&key).unwrap();

    let reopened = SearchIndex::open(&blob, &key).unwrap();
    assert_eq!(reopened.len(), 3);
    assert_eq!(
        search_ids(&reopened, &SearchQuery::new().keywords("discount")),
        Vec::<String>::new()
    );
    assert_eq!(
        search_ids(&reopened, &SearchQuery::new().keywords("résumé")),
        vec!["m1"]
    );

    let failures = [
        SearchIndex::open(&blob, &SymmetricKey::generate()),
        SearchIndex::open(&blob[..blob.len() - 1], &key),
        SearchIndex::open(b"not an index", &key),
    ];
    for failure in failures {
        assert!(
            matches!(failure, Err(Error::IndexDecryption)),
            "{failure:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn indexer_batches_are_resumable() {
    let order = ["m1", "m2", "m3"];
    let reader = std::sync::Arc::new(std::sync::OnceLock::<rikka_proton_pgp::PublicKey>::new());
    let sealing = std::sync::Arc::clone(&reader);

    let signed = signed_in(move |request: Request| {
        let page_size: usize = request
            .query("PageSize")
            .and_then(|size| size.parse().ok())
            .unwrap_or(50);
        match request.route() {
            ("GET", "/mail/v4/messages") => {
                let start = request
                    .query("EndID")
                    .and_then(|end| order.iter().position(|id| *id == end))
                    .map_or(0, |at| at + 1);
                let listed: Vec<Value> = order
                    .iter()
                    .enumerate()
                    .skip(start)
                    .take(page_size)
                    .map(|(at, id)| listing(id, at))
                    .collect();
                json_response(200, &json!({"Code": 1000, "Messages": listed}))
            }
            ("GET", path) if path.starts_with("/mail/v4/messages/") => {
                let id = path.trim_start_matches("/mail/v4/messages/").to_owned();
                let at = order
                    .iter()
                    .position(|listed_id| *listed_id == id)
                    .unwrap_or(0);
                let body = Pgp::new()
                    .encrypt()
                    .to(sealing.get().unwrap().clone())
                    .armored_message(format!("secret body {id}"))
                    .unwrap();
                let mut message = listing(&id, at);
                message["Body"] = json!(body);
                message["MIMEType"] = json!("text/plain");
                json_response(200, &json!({"Code": 1000, "Message": message}))
            }
            _ => not_found(),
        }
    })
    .await;
    reader.set(signed.address_key.public_key()).unwrap();

    let indexer = signed
        .client
        .indexer()
        .batch_size(2)
        .pause(std::time::Duration::ZERO);
    let key = SymmetricKey::generate();

    let mut index = SearchIndex::new();
    let first = indexer.step(&mut index).await.unwrap();
    assert_eq!(first, IndexStep::Indexed(2));
    assert!(!index.is_complete());

    let mut resumed = SearchIndex::open(&index.seal(&key).unwrap(), &key).unwrap();
    let mut steps = Vec::new();
    indexer
        .run(&mut resumed, |_, step| {
            steps.push(step);
            std::ops::ControlFlow::Continue(())
        })
        .await
        .unwrap();

    assert_eq!(steps, vec![IndexStep::Indexed(1), IndexStep::Done]);
    assert!(resumed.is_complete());
    assert_eq!(resumed.len(), 3);
    assert_eq!(
        search_ids(&resumed, &SearchQuery::new().keywords("secret body m3")),
        vec!["m3"]
    );
    assert_eq!(
        search_ids(&resumed, &SearchQuery::new().keywords("secret body")),
        vec!["m1", "m2", "m3"]
    );
}

fn listing(id: &str, at: usize) -> Value {
    json!({
        "ID": id,
        "AddressID": ADDRESS_ID,
        "Subject": format!("subject {id}"),
        "Sender": {"Address": EMAIL, "Name": "Tester"},
        "LabelIDs": ["0", "5"],
        "Time": 1000 - at,
        "Flags": 1
    })
}
