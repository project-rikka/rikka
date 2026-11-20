use std::sync::{Arc, Mutex};

use rikka_proton_cache::Cache;
use rikka_proton_contacts::{
    ContactBook, ContactEvents, ContactId, EmailPreferences, EventId, GroupId, MailDefaults,
    MessageIntent, PackageScheme, Property, VCard, seal_vcard,
};
use rikka_proton_pgp::{Pgp, SymmetricKey};
use serde_json::{Value, json};

use crate::support::signed_in;
use rikka_test_proton::{Record, Request, json_response, new_key, not_found};

fn contact_json(id: &str, cards: &Value) -> Value {
    json!({
        "ID": id,
        "Name": "Ada",
        "UID": format!("uid-{id}"),
        "ContactEmails": [],
        "LabelIDs": [],
        "Cards": cards
    })
}

#[tokio::test]
async fn contacts_page_until_total() {
    let record = Record::default();
    let log = record.clone();
    let signed = signed_in(move |request: Request| {
        log.push(&request);
        match request.route() {
            ("GET", "/contacts/v4/contacts") => {
                let page = if request.query.contains("Page=0") {
                    json!([{"ID": "a"}, {"ID": "b"}])
                } else {
                    json!([{"ID": "c"}])
                };
                json_response(200, &json!({"Code": 1000, "Contacts": page, "Total": 3}))
            }
            _ => not_found(),
        }
    })
    .await;

    let contacts = signed
        .client
        .contacts()
        .in_group(&GroupId::from("group"))
        .page_size(2)
        .list()
        .await
        .unwrap();

    assert_eq!(
        contacts
            .iter()
            .map(|contact| contact.id().as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b", "c"]
    );
    let request = record.requests();
    let pages: Vec<&Request> = request
        .iter()
        .filter(|request| request.path == "/contacts/v4/contacts")
        .collect();
    assert_eq!(pages.len(), 2);
    assert!(pages[0].query.contains("LabelID=group"));
    assert!(pages[1].query.contains("Page=1"));
}

#[tokio::test]
async fn created_contact_cards_encrypted_and_signed() {
    let record = Record::default();
    let log = record.clone();
    let signed = signed_in(move |request: Request| {
        log.push(&request);
        match request.route() {
            ("POST", "/contacts/v4/contacts") => json_response(
                200,
                &json!({
                    "Code": 1001,
                    "Responses": [{"Index": 0, "Response": {"Code": 1000, "Contact": {"ID": "new"}}}]
                }),
            ),
            _ => not_found(),
        }
    })
    .await;

    let card = VCard::new()
        .with(Property::text("FN", "Ada"))
        .with(Property::text("EMAIL", "ada@example.com"))
        .with(Property::text("TEL", "+1"));
    let outcomes = signed
        .client
        .create([card])
        .groups_from_categories(true)
        .execute()
        .await
        .unwrap();

    assert!(outcomes[0].is_ok());
    assert_eq!(
        outcomes[0]
            .body()
            .contact()
            .map(|contact| contact.id().as_str()),
        Some("new")
    );

    let body = record
        .requests()
        .iter()
        .find(|request| request.method == "POST")
        .map(Request::json)
        .unwrap();
    assert_eq!(body["Overwrite"], 0);
    assert_eq!(body["Labels"], 1);
    let kinds: Vec<i64> = body["Contacts"][0]["Cards"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|card| card["Type"].as_i64())
        .collect();
    assert_eq!(kinds, vec![3, 2]);
}

#[tokio::test]
async fn pinned_keys_drive_send_preferences() {
    let pgp = Pgp::new();
    let pinned = new_key(&pgp, "bob@example.com").public_key();
    let cards: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
    let served = cards.clone();
    let signed = signed_in(move |request: Request| match request.route() {
        ("GET", "/contacts/v4/contacts/emails") => json_response(
            200,
            &json!({
                "Code": 1000,
                "Total": 1,
                "ContactEmails": [{"ID": "email", "Email": "bob@example.com", "ContactID": "bob", "Defaults": 0}]
            }),
        ),
        ("GET", "/contacts/v4/contacts/bob") => {
            let cards = served.lock().unwrap().clone().unwrap_or(Value::Null);
            json_response(200, &json!({"Code": 1000, "Contact": contact_json("bob", &cards)}))
        }
        ("GET", "/core/v4/keys/all") => json_response(422, &json!({"Code": 33103, "Error": "External address"})),
        _ => not_found(),
    })
    .await;

    let mut card = VCard::new()
        .with(Property::text("FN", "Bob"))
        .with(Property::text("EMAIL", "bob@example.com").with_group("item1"));
    EmailPreferences {
        pinned_keys: vec![pinned.clone()],
        encrypt_to_pinned: Some(true),
        ..EmailPreferences::default()
    }
    .apply_to(&mut card, "bob@example.com")
    .unwrap();
    let sealed = seal_vcard(&pgp, &card, &signed.user_key).unwrap();
    *cards.lock().unwrap() = Some(serde_json::to_value(&sealed).unwrap());

    let keys = signed.client.pinned_keys("Bob@Example.com").await;
    assert!(keys.is_contact());
    assert_eq!(keys.signature_verified(), Some(true));
    assert_eq!(keys.preferences().pinned_keys.len(), 1);

    let send = signed
        .client
        .send_preferences(
            "bob@example.com",
            &MailDefaults::default(),
            &MessageIntent::default(),
        )
        .await
        .unwrap();
    assert_eq!(send.scheme, PackageScheme::PgpMime);
    assert!(send.encrypt);
    assert!(send.key_pinned);
    assert_eq!(
        send.key.map(|key| key.fingerprint()),
        Some(pinned.fingerprint())
    );
}

#[tokio::test]
async fn merge_updates_first_deletes_rest() {
    let pgp = Pgp::new();
    let record = Record::default();
    let log = record.clone();
    let stored: Arc<Mutex<Vec<(String, Value)>>> = Arc::new(Mutex::new(Vec::new()));
    let served = stored.clone();
    let signed = signed_in(move |request: Request| {
        log.push(&request);
        let path = request.path.clone();
        match request.route() {
            ("GET", _) if path.starts_with("/contacts/v4/contacts/") => {
                let id = path.trim_start_matches("/contacts/v4/contacts/").to_owned();
                let cards = served
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|(contact_id, _)| *contact_id == id)
                    .map(|(_, cards)| cards.clone())
                    .unwrap_or(Value::Null);
                json_response(
                    200,
                    &json!({"Code": 1000, "Contact": contact_json(&id, &cards)}),
                )
            }
            ("PUT", "/contacts/v4/contacts/first") => {
                json_response(200, &json!({"Code": 1000, "Contact": {"ID": "first"}}))
            }
            ("PUT", "/contacts/v4/contacts/delete") => json_response(
                200,
                &json!({"Code": 1001, "Responses": [{"ID": "second", "Response": {"Code": 1000}}]}),
            ),
            _ => not_found(),
        }
    })
    .await;

    for (id, phone) in [("first", "+1 111"), ("second", "+1 222")] {
        let card = VCard::new()
            .with(Property::text("FN", id))
            .with(Property::text("EMAIL", "same@example.com"))
            .with(Property::text("TEL", phone));
        let cards = seal_vcard(&pgp, &card, &signed.user_key).unwrap();
        stored
            .lock()
            .unwrap()
            .push((id.to_owned(), serde_json::to_value(&cards).unwrap()));
    }

    let merged = signed
        .client
        .merge(&[ContactId::from("first"), ContactId::from("second")])
        .await
        .unwrap();
    assert_eq!(merged.id().as_str(), "first");

    let request = record.requests();
    let deleted = request
        .iter()
        .find(|request| request.path == "/contacts/v4/contacts/delete")
        .map(Request::json)
        .unwrap();
    assert_eq!(deleted["IDs"], json!(["second"]));
    let update = request
        .iter()
        .find(|request| request.method == "PUT" && request.path == "/contacts/v4/contacts/first")
        .map(Request::json)
        .unwrap();
    let cards: Vec<rikka_proton_contacts::ContactCard> =
        serde_json::from_value(update["Cards"].clone()).unwrap();
    let decrypted = rikka_proton_contacts::decrypt_cards(
        &pgp,
        &cards,
        &rikka_proton_pgp::KeyRing::new().with_private(signed.user_key.clone()),
    );
    assert_eq!(decrypted.vcard().formatted_name().as_deref(), Some("first"));
    assert_eq!(decrypted.vcard().properties_named("TEL").count(), 2);
    assert_eq!(decrypted.vcard().emails().count(), 1);
}

#[tokio::test]
async fn events_update_contact_book() {
    let signed = signed_in(|request: Request| match request.route() {
        ("GET", "/core/v4/events/first") => json_response(
            200,
            &json!({
                "Code": 1000,
                "EventID": "second",
                "More": 0,
                "Contacts": [{"ID": "c1", "Action": 1, "Contact": {"ID": "c1", "Name": "Ada"}}],
                "ContactEmails": [{"ID": "e1", "Action": 1, "ContactEmail": {"ID": "e1", "Email": "ada@example.com", "ContactID": "c1"}}],
                "Labels": [
                    {"ID": "g1", "Action": 1, "Label": {"ID": "g1", "Name": "Friends", "Type": 2}},
                    {"ID": "f1", "Action": 1, "Label": {"ID": "f1", "Name": "Folder", "Type": 3}}
                ]
            }),
        ),
        ("GET", "/core/v4/events/second") => json_response(
            200,
            &json!({
                "Code": 1000,
                "EventID": "third",
                "Contacts": [{"ID": "c1", "Action": 0}],
                "Labels": [{"ID": "g1", "Action": 0}]
            }),
        ),
        ("GET", "/core/v4/events/third") => {
            json_response(200, &json!({"Code": 1000, "EventID": "fourth", "Refresh": 2}))
        }
        _ => not_found(),
    })
    .await;

    let mut book = ContactBook::new();
    let first: ContactEvents = signed
        .account
        .core_events(&EventId::from("first"))
        .await
        .unwrap();
    first.apply(&mut book);
    assert_eq!(first.id().as_str(), "second");
    assert_eq!(book.contacts().count(), 1);
    assert_eq!(book.emails_for(&ContactId::from("c1")).count(), 1);
    assert_eq!(
        book.groups().map(|group| group.name()).collect::<Vec<_>>(),
        vec!["Friends"]
    );

    signed
        .account
        .core_events::<ContactEvents>(first.id())
        .await
        .unwrap()
        .apply(&mut book);
    assert_eq!(book.contacts().count(), 0);
    assert_eq!(book.emails().count(), 0);
    assert_eq!(book.groups().count(), 0);

    let refresh: ContactEvents = signed
        .account
        .core_events(&EventId::from("third"))
        .await
        .unwrap();
    assert!(refresh.needs_reload());
    refresh.apply(&mut book);
    assert!(book.is_stale());
}

#[tokio::test(flavor = "multi_thread")]
async fn events_and_reads_update_cache() {
    let signed = signed_in(|request: Request| match request.route() {
        ("GET", "/contacts/v4/contacts/c2") => json_response(
            200,
            &json!({"Code": 1000, "Contact": contact_json("c2", &json!([{"Type": 0, "Data": "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Bea\r\nEND:VCARD"}]))}),
        ),
        ("GET", "/core/v4/events/first") => json_response(
            200,
            &json!({
                "Code": 1000,
                "EventID": "second",
                "Contacts": [{"ID": "c1", "Action": 1, "Contact": {"ID": "c1", "Name": "Ada"}}],
                "ContactEmails": [{"ID": "e1", "Action": 1, "ContactEmail": {"ID": "e1", "Email": "ada@example.com", "ContactID": "c1"}}]
            }),
        ),
        ("GET", "/core/v4/events/second") => json_response(
            200,
            &json!({"Code": 1000, "EventID": "third", "Contacts": [{"ID": "c1", "Action": 0}]}),
        ),
        _ => not_found(),
    })
    .await;
    let cache = Cache::in_memory(SymmetricKey::generate()).unwrap();
    let client = signed.client.with_cache(cache.clone());
    assert!(client.cached_book().is_none());

    client.contact(&ContactId::from("c2")).await.unwrap();
    let first: ContactEvents = signed
        .account
        .core_events(&EventId::from("first"))
        .await
        .unwrap();
    client.apply_events(&first);
    cache.flush().unwrap();

    let book = client.cached_book().unwrap();
    assert_eq!(book.contacts().count(), 2);
    assert_eq!(book.emails_for(&ContactId::from("c1")).count(), 1);
    assert!(
        client
            .cached_contact(&ContactId::from("c2"))
            .is_some_and(|contact| contact.has_cards())
    );

    let second: ContactEvents = signed.account.core_events(first.id()).await.unwrap();
    client.apply_events(&second);
    let book = client.cached_book().unwrap();
    assert_eq!(book.contacts().count(), 1);
    assert_eq!(book.emails().count(), 0);
}
#[tokio::test]
async fn groups_are_contact_labels() {
    let record = Record::default();
    let log = record.clone();
    let signed = signed_in(move |request: Request| {
        log.push(&request);
        match request.route() {
            ("GET", "/core/v4/labels") => json_response(
                200,
                &json!({"Code": 1000, "Labels": [{"ID": "g1", "Name": "Friends", "Type": 2}]}),
            ),
            ("POST", "/core/v4/labels") => json_response(
                200,
                &json!({"Code": 1000, "Label": {"ID": "g2", "Name": "Family", "Type": 2, "Color": "#112233"}}),
            ),
            _ => not_found(),
        }
    })
    .await;

    let groups = signed.client.groups().await.unwrap();
    assert_eq!(groups[0].id().as_str(), "g1");

    let created = signed
        .client
        .new_group("Family")
        .color("#112233")
        .create()
        .await
        .unwrap();
    assert_eq!(created.name(), "Family");
    assert!(created.is_contact_group());

    assert!(signed.client.new_group("  ").create().await.is_err());

    let request = record.requests();
    assert!(
        request
            .iter()
            .any(|request| request.method == "GET" && request.query.contains("Type=2"))
    );
    let posted = request
        .iter()
        .find(|request| request.method == "POST")
        .map(Request::json)
        .unwrap();
    assert_eq!(
        posted,
        json!({"Name": "Family", "Color": "#112233", "Type": 2})
    );
}

#[tokio::test]
async fn missing_address_is_send_error() {
    let signed = signed_in(|request: Request| match request.route() {
        ("GET", "/core/v4/keys/all") => {
            json_response(422, &json!({"Code": 33102, "Error": "Address missing"}))
        }
        _ => not_found(),
    })
    .await;

    let send = signed
        .client
        .send_preferences(
            "ghost@proton.test",
            &MailDefaults::default(),
            &MessageIntent::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        send.error,
        Some(rikka_proton_contacts::PreferenceError::EmailAddress(
            "Address missing".to_owned()
        ))
    );

    let verification = signed
        .client
        .verification_keys("ghost@proton.test")
        .await
        .unwrap();
    assert!(verification.keys.is_empty());
    assert_eq!(verification.errors, ["Address missing"]);
}
