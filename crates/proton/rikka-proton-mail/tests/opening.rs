use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::Notify;

use crate::support::signed_in;
use rikka_proton_mail::MessageId;
use rikka_proton_pgp::{Pgp, PrivateKey, PublicKey};
use rikka_test_proton::{ADDRESS_ID, Request, ResponseFuture, json_response, new_key, not_found};
use serde_json::{Value, json};

const STRANGER: &str = "ana@elsewhere.test";
const PLAINTEXT: &str = "see you at noon";
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Default)]
struct Counters {
    lookups: AtomicUsize,
    lookup_seen: Notify,
    messages: AtomicUsize,
    responded_before_lookup: AtomicBool,
}

impl Counters {
    fn lookups(&self) -> usize {
        self.lookups.load(Ordering::SeqCst)
    }

    fn messages(&self) -> usize {
        self.messages.load(Ordering::SeqCst)
    }

    fn record_lookup(&self) {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        self.lookup_seen.notify_waiters();
    }
}

fn encrypt_signed(pgp: &Pgp, reader: &PrivateKey, writer: &PrivateKey) -> String {
    pgp.encrypt()
        .to(reader.public_key())
        .signed_by(writer.clone())
        .armored_message(PLAINTEXT)
        .unwrap()
}

fn message(id: &str, sender: &str, body: &str) -> Value {
    json!({
        "Code": 1000,
        "Message": {
            "ID": id,
            "AddressID": ADDRESS_ID,
            "Subject": "Lunch",
            "Sender": {"Address": sender, "Name": "Ana"},
            "Body": body,
            "MIMEType": "text/plain"
        }
    })
}

fn known_keys(key: &PublicKey) -> Value {
    json!({
        "Code": 1000,
        "Address": {"Keys": [{
            "Flags": 3,
            "PublicKey": key.export_armored().unwrap()
        }]}
    })
}

fn respond_after_lookup(counters: &Arc<Counters>, reply: Value) -> ResponseFuture {
    let counters = Arc::clone(counters);
    Box::pin(async move {
        let lookup_seen = counters.lookup_seen.notified();
        if counters.lookups() == 0
            && tokio::time::timeout(LOOKUP_TIMEOUT, lookup_seen)
                .await
                .is_err()
        {
            counters
                .responded_before_lookup
                .store(true, Ordering::SeqCst);
        }
        json_response(200, &reply).await
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn external_sender_verifies_with_published_keys() {
    let pgp = Pgp::new();
    let stranger = new_key(&pgp, STRANGER);
    let counters = Arc::new(Counters::default());
    let watched = Arc::clone(&counters);
    let keys = known_keys(&stranger.public_key());
    let reader: Arc<std::sync::OnceLock<PrivateKey>> = Arc::default();
    let reader_key = Arc::clone(&reader);
    let writer = stranger.clone();

    let signed = signed_in(move |request: Request| match request.route() {
        ("GET", "/core/v4/keys/all") => {
            watched.record_lookup();
            match request.query("InternalOnly") {
                Some("1") => json_response(200, &json!({"Code": 1000, "Address": {"Keys": []}})),
                _ => json_response(200, &keys),
            }
        }
        ("GET", path) if path.starts_with("/mail/v4/messages/") => {
            watched.messages.fetch_add(1, Ordering::SeqCst);
            let id = path.trim_start_matches("/mail/v4/messages/");
            let body = encrypt_signed(&Pgp::new(), reader_key.get().unwrap(), &writer);
            respond_after_lookup(&watched, message(id, STRANGER, &body))
        }
        _ => not_found(),
    })
    .await;
    reader.set(signed.address_key.clone()).unwrap();

    let first = signed
        .client
        .open_message(&MessageId::from("msg-1"), Some(STRANGER))
        .await
        .unwrap();
    let again = signed
        .client
        .clone()
        .open_message(&MessageId::from("msg-2"), Some(STRANGER))
        .await
        .unwrap();

    assert_eq!(first.decrypted().text(), PLAINTEXT);
    assert!(first.decrypted().is_verified());
    assert!(again.decrypted().is_verified());
    assert_eq!(first.message().id().as_str(), "msg-1");
    assert!(
        !counters.responded_before_lookup.load(Ordering::SeqCst),
        "sender keys not fetched concurrently with message"
    );
    assert_eq!(counters.messages(), 2);
    assert_eq!(counters.lookups(), 1, "clones do not share sender keys");
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_sender_still_decrypts() {
    let pgp = Pgp::new();
    let stranger = new_key(&pgp, STRANGER);
    let counters = Arc::new(Counters::default());
    let watched = Arc::clone(&counters);
    let body: Arc<std::sync::OnceLock<String>> = Arc::default();
    let served = Arc::clone(&body);

    let signed = signed_in(move |request: Request| match request.route() {
        ("GET", "/core/v4/keys/all") => {
            watched.record_lookup();
            rikka_test_proton::response(422, r#"{"Code":33102,"Error":"No such address"}"#)
        }
        ("GET", "/mail/v4/messages/msg-1") => {
            json_response(200, &message("msg-1", STRANGER, served.get().unwrap()))
        }
        _ => not_found(),
    })
    .await;
    body.set(encrypt_signed(&pgp, &signed.address_key, &stranger))
        .unwrap();

    let message = signed
        .client
        .message(&MessageId::from("msg-1"))
        .await
        .unwrap();
    let decrypted_message = signed.client.decrypt_message(&message).await.unwrap();
    let again = signed.client.decrypt_message(&message).await.unwrap();

    assert_eq!(decrypted_message.text(), PLAINTEXT);
    assert!(!decrypted_message.is_verified());
    assert!(!again.is_verified());
    assert_eq!(counters.lookups(), 1, "refusal not cached as no keys");
}
