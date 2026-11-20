use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use proton_mailbox::{
    Body, Error, OpenedMessage, OpenedThread, Person, Plain, RowId, ThreadCache, Verification,
};
use rikka_proton_account::AddressId;
use rikka_proton_mail::{Event, MessageId};

fn opened(id: &str) -> OpenedMessage {
    OpenedMessage {
        id: MessageId::from(id),
        address: AddressId::from("address-1"),
        subject: "Lunch".to_owned(),
        sender: Person::new("ana@proton.me", "Ana"),
        to: Vec::new(),
        cc: Vec::new(),
        bcc: Vec::new(),
        reply_to: Vec::new(),
        time: jiff::Timestamp::from_second(1_700_000_000).unwrap(),
        unread: false,
        starred: false,
        draft: false,
        words: Body::Plain(Plain::new("see you at noon")),
        signature: Verification::Verified,
        files: Vec::new(),
        labels: Vec::new(),
        external: None,
    }
}

fn message(id: &str) -> RowId {
    RowId::Message(MessageId::from(id))
}

fn thread(id: &str) -> RowId {
    RowId::Conversation(id.into())
}

fn reading(id: &RowId, messages: &[&str]) -> OpenedThread {
    OpenedThread {
        id: id.clone(),
        subject: "Lunch".to_owned(),
        messages: messages.iter().map(|message| opened(message)).collect(),
        thread: i64::try_from(messages.len()).expect("a small thread"),
    }
}

fn event(body: &str) -> Event {
    serde_json::from_str(body).expect("an event decodes")
}

struct Counted {
    fetched: Arc<AtomicUsize>,
}

impl Counted {
    fn new() -> Counted {
        Counted {
            fetched: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn fetch(
        &self,
        id: &RowId,
        messages: &'static [&'static str],
    ) -> impl Future<Output = proton_mailbox::Result<OpenedThread>> + Send + 'static {
        let fetched = Arc::clone(&self.fetched);
        let id = id.clone();
        async move {
            fetched.fetch_add(1, Ordering::SeqCst);
            tokio::task::yield_now().await;
            Ok(reading(&id, messages))
        }
    }

    fn count(&self) -> usize {
        self.fetched.load(Ordering::SeqCst)
    }
}

async fn read(readings: &ThreadCache, id: &RowId, counted: &Counted) -> Arc<OpenedThread> {
    readings
        .read(id, counted.fetch(id, &["m-1"]))
        .await
        .expect("the reading arrives")
}

#[tokio::test]
async fn reading_is_cached() {
    let readings = ThreadCache::new();
    let counted = Counted::new();
    let id = message("m-1");

    let first = read(&readings, &id, &counted).await;
    let again = read(&readings, &id, &counted).await;

    assert_eq!(counted.count(), 1);
    assert!(Arc::ptr_eq(&first, &again));
    assert!(readings.get(&id).is_some());
}

#[tokio::test]
async fn failed_reading_is_retried() {
    let readings = ThreadCache::new();
    let counted = Counted::new();
    let id = message("m-1");

    let refused = readings.read(&id, async { Err(Error::NoRecipient) }).await;

    assert!(refused.is_err());
    assert!(!readings.contains(&id));
    read(&readings, &id, &counted).await;
    assert_eq!(counted.count(), 1);
}

#[tokio::test]
async fn changed_message_evicts_readings() {
    let readings = ThreadCache::new();
    let counted = Counted::new();
    let lone = message("m-2");
    let conversation = thread("c-1");
    readings
        .read(&conversation, counted.fetch(&conversation, &["m-1", "m-2"]))
        .await
        .expect("the thread arrives");
    read(&readings, &lone, &counted).await;

    readings.apply_event(&event(
        r#"{"EventID":"e-1","Messages":[{"ID":"m-2","Action":2}]}"#,
    ));

    assert!(!readings.contains(&lone));
    assert!(!readings.contains(&conversation));
}
