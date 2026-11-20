use proton_mailbox::{Ask, Kind, Listing, Mailbox, Row, arranged};
use rikka_proton_mail::{Event, Label, MessageMetadata, SystemLabel};

fn message(id: &str, unread: bool) -> MessageMetadata {
    serde_json::from_str(&format!(
        r#"{{"ID":"{id}","ConversationID":"thread-1","Subject":"Lunch","Unread":{},
            "Sender":{{"Address":"ana@proton.me","Name":"Ana"}},
            "ToList":[{{"Address":"me@proton.me","Name":"Me"}}],
            "Time":1700000000,"NumAttachments":1,"LabelIDs":["0","5"],"Size":2048}}"#,
        i32::from(unread)
    ))
    .expect("a message decodes")
}

fn event(body: &str) -> Event {
    serde_json::from_str(body).expect("an event decodes")
}

#[test]
fn next_page_appends_rows() {
    let mut mailbox = Mailbox::new();
    let ask = Ask::new(SystemLabel::Inbox.id(), Kind::Inbox, false);
    mailbox.apply_listing(
        Listing {
            rows: vec![Row::of_message(&message("message-1", true), false)],
            more: true,
        },
        &ask,
    );

    mailbox.apply_listing(
        Listing {
            rows: vec![
                Row::of_message(&message("message-1", true), false),
                Row::of_message(&message("message-2", true), false),
            ],
            more: false,
        },
        &ask.clone().at_page(1),
    );

    assert_eq!(mailbox.rows().len(), 2);
    assert_eq!(mailbox.page(), 1);
    assert!(!mailbox.has_more());
}

#[test]
fn message_event_reloads_listing() {
    let mut mailbox = Mailbox::new();

    let invalidation = mailbox.apply_event(
        &event(r#"{"EventID":"event-2","Messages":[{"ID":"message-1","Action":2}]}"#),
        false,
    );

    assert!(invalidation.listing);
    assert!(invalidation.folders);
}

#[test]
fn counted_event_updates_folders_without_reloading_them() {
    let mut mailbox = Mailbox::new();
    mailbox.set_folders(arranged(&[], &[]));

    let invalidation = mailbox.apply_event(
        &event(
            r#"{"EventID":"event-3","Messages":[{"ID":"message-1","Action":2}],
                "MessageCounts":[{"LabelID":"0","Total":7,"Unread":3}]}"#,
        ),
        false,
    );

    assert!(invalidation.listing);
    assert!(!invalidation.folders);
    let inbox = &mailbox.folders()[0];
    assert_eq!((inbox.total, inbox.unread), (7, 3));
}

#[test]
fn refresh_event_reloads_everything() {
    let mut mailbox = Mailbox::new();

    let invalidation = mailbox.apply_event(&event(r#"{"EventID":"event-4","Refresh":1}"#), false);

    assert!(invalidation.folders);
    assert!(invalidation.listing);
}

fn label(id: &str, name: &str, kind: i32, parent: Option<&str>, order: i32) -> Label {
    let parent = parent.map_or_else(|| "null".to_owned(), |id| format!("\"{id}\""));
    serde_json::from_str(&format!(
        r#"{{"ID":"{id}","Name":"{name}","Type":{kind},"ParentID":{parent},"Order":{order},"Color":"red"}}"#
    ))
    .expect("a label decodes")
}

#[test]
fn orphan_folder_is_listed() {
    let labels = [label("bills", "Bills", 3, Some("gone"), 1)];

    let folders = arranged(&labels, &[]);

    assert!(folders.iter().any(|folder| folder.name == "Bills"));
}
