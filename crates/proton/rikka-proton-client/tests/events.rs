use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rikka_proton_client::{Change, Session, Watch};
use rikka_proton_contacts::{ContactEvents, ContactsClient};
use rikka_proton_mail::{Event, MailClient, MessageId};
use rikka_proton_pgp::SymmetricKey;
use serde_json::json;

use crate::support::{TestAccount, accounts, signed_in};
use rikka_test_proton::{Record, Request, json_response, not_found, serve};

const TIMEOUT: Duration = Duration::from_secs(20);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Debug)]
enum Product {
    Mail(Box<Event>),
    Contacts(Box<ContactEvents>),
}

struct Products {
    mail: MailClient,
    contacts: ContactsClient,
}

impl Products {
    fn of(session: &Session) -> Products {
        let account = session.proton().account();
        Products {
            mail: MailClient::for_account(account, session.keys()),
            contacts: ContactsClient::for_account(account, session.keys()),
        }
    }

    fn watch(&self, session: &Session) -> Watch<Product> {
        session
            .event_loop()
            .every(POLL_INTERVAL)
            .feed(self.mail.clone(), Product::Mail)
            .feed(self.contacts.clone(), Product::Contacts)
            .start()
    }
}

fn message(id: &str) -> serde_json::Value {
    json!({
        "ID": id,
        "ConversationID": "conversation",
        "AddressID": "alice-address",
        "Subject": "hello",
        "Sender": {"Address": "friend@proton.test", "Name": "Friend"},
        "LabelIDs": ["0"],
        "Time": 1_700_000_000
    })
}

async fn next_matching<C>(watch: &mut Watch<C>, predicate: impl Fn(&Change<C>) -> bool) -> Change<C>
where
    C: Send + 'static,
{
    tokio::time::timeout(TIMEOUT, async {
        loop {
            let change = watch.next().await.unwrap();
            if predicate(&change) {
                return change;
            }
        }
    })
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn core_events_update_caches_and_resume() {
    let alice = Arc::new(TestAccount::new("alice", "alice@proton.test"));
    let record = Record::default();
    let address = {
        let (alice, record) = (Arc::clone(&alice), record.clone());
        serve(move |request: Request| {
            record.push(&request);
            if let Some(response) = alice.respond(&request) {
                return response;
            }
            match request.route() {
                ("GET", "/core/v4/events/latest") => json_response(200, &json!({"Code": 1000, "EventID": "e1"})),
                ("GET", "/core/v4/events/e1") => json_response(
                    200,
                    &json!({
                        "Code": 1000,
                        "EventID": "e2",
                        "More": 1,
                        "Messages": [{"ID": "m1", "Action": 1, "Message": message("m1")}]
                    }),
                ),
                ("GET", "/core/v4/events/e2") => json_response(
                    200,
                    &json!({
                        "Code": 1000,
                        "EventID": "e3",
                        "Contacts": [{"ID": "c1", "Action": 1, "Contact": {"ID": "c1", "Name": "Ada"}}]
                    }),
                ),
                ("GET", "/core/v4/events/e3") => json_response(200, &json!({"Code": 1000, "EventID": "e3"})),
                _ => not_found(),
            }
        })
        .await
    };
    let root = tempfile::tempdir().unwrap();
    let key = SymmetricKey::generate();
    let book = accounts(&address, root.path(), key.clone()).await;
    let session = signed_in(&book, &alice).await;

    let products = Products::of(&session);
    let mut watch = products.watch(&session);
    let mail = next_matching(&mut watch, |change| {
        matches!(change, Change::Product(Product::Mail(_)))
    })
    .await;
    let Change::Product(Product::Mail(event)) = mail else {
        unreachable!()
    };
    assert_eq!(event.messages()[0].id(), &MessageId::from("m1"));
    let contacts = next_matching(&mut watch, |change| {
        matches!(change, Change::Product(Product::Contacts(_)))
    })
    .await;
    let Change::Product(Product::Contacts(events)) = contacts else {
        unreachable!()
    };
    assert_eq!(events.contacts().len(), 1);
    assert!(
        products
            .mail
            .cached_message(&MessageId::from("m1"))
            .is_some()
    );
    assert!(
        products
            .contacts
            .cached_book()
            .is_some_and(|book| book.contacts().count() == 1)
    );
    drop(watch);
    session.proton().account().cache().unwrap().flush().unwrap();
    drop((session, products, book));

    let requests_before = record.count("GET", "/core/v4/events/e3");
    let book = accounts(&address, root.path(), key).await;
    let user = book.primary().unwrap();
    let session = book
        .resume(&user, rikka_test_proton::PASSPHRASE)
        .await
        .unwrap();
    let mut watch = Products::of(&session).watch(&session);
    tokio::time::timeout(TIMEOUT, async {
        while record.count("GET", "/core/v4/events/e3") <= requests_before {
            drop(tokio::time::timeout(POLL_INTERVAL, watch.next()).await);
        }
    })
    .await
    .unwrap();
    drop(watch);
    assert_eq!(record.count("GET", "/core/v4/events/latest"), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn expired_session_ends_watch() {
    let alice = Arc::new(TestAccount::new("alice", "alice@proton.test"));
    let expired = Arc::new(AtomicBool::new(false));
    let address = {
        let (alice, expired) = (Arc::clone(&alice), Arc::clone(&expired));
        serve(move |request: Request| {
            if request.route() == ("POST", "/auth/v4/refresh") {
                return json_response(
                    422,
                    &json!({"Code": 10013, "Error": "Invalid refresh token"}),
                );
            }
            if expired.load(Ordering::SeqCst) {
                return json_response(401, &json!({"Code": 401, "Error": "Invalid access token"}));
            }
            alice.respond(&request).unwrap_or_else(not_found)
        })
        .await
    };
    let root = tempfile::tempdir().unwrap();
    let book = accounts(&address, root.path(), SymmetricKey::generate()).await;
    let session = signed_in(&book, &alice).await;
    expired.store(true, Ordering::SeqCst);

    let mut watch =
        std::thread::spawn(move || session.event_loop::<Product>().every(POLL_INTERVAL).start())
            .join()
            .unwrap();
    next_matching(&mut watch, |change| matches!(change, Change::SignedOut)).await;
    assert!(
        tokio::time::timeout(TIMEOUT, watch.next())
            .await
            .unwrap()
            .is_none()
    );
}
