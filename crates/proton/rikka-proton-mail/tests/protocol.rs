use jiff::Timestamp;
use rikka_proton_mail::{
    Conversation, Event, EventAction, Label, Message, MessageFlags, MessageMetadata, MimeType,
    SystemLabel,
};
use serde_json::{Value, json};

fn read<T: serde::de::DeserializeOwned>(wire: &Value) -> T {
    serde_json::from_value(wire.clone()).unwrap()
}

#[test]
fn listed_message_keeps_every_field() {
    let wire = json!({
        "ID": "msg-1",
        "ConversationID": "conv-1",
        "AddressID": "addr-1",
        "Subject": "Quarterly report",
        "Sender": {"Address": "boss@proton.me", "Name": "Boss", "IsProton": 1},
        "ToList": [{"Address": "me@proton.me", "Name": "Me"}],
        "CCList": [{"Address": "team@proton.me", "Name": ""}],
        "BCCList": [],
        "ReplyTos": [{"Address": "boss@proton.me", "Name": "Boss"}],
        "LabelIDs": ["0", "5", "10"],
        "ExternalID": "<abc@mail.test>",
        "Flags": 8193,
        "Time": 1_700_000_000,
        "Size": 4096,
        "Order": 42,
        "ExpirationTime": 0,
        "NumAttachments": 1,
        "Unread": 1,
        "IsReplied": 0,
        "IsRepliedAll": 0,
        "IsForwarded": 1
    });

    let message: MessageMetadata = read(&wire);

    assert_eq!(message.id().as_str(), "msg-1");
    assert_eq!(message.conversation_id().as_str(), "conv-1");
    assert_eq!(message.address_id().as_str(), "addr-1");
    assert_eq!(message.subject(), "Quarterly report");
    assert!(message.sender().is_proton());
    assert_eq!(message.cc().len(), 1);
    assert_eq!(message.reply_tos()[0].name(), "Boss");
    assert_eq!(message.external_id(), Some("<abc@mail.test>"));
    assert!(message.is_unread());
    assert!(message.is_forwarded());
    assert!(!message.is_replied());
    assert!(message.is_starred());
    assert!(message.has_label(&SystemLabel::AllMail.id()));
    assert!(message.flags().contains(MessageFlags::RECEIVED));
    assert!(message.flags().contains(MessageFlags::TOUCHED));
    assert!(!message.is_draft());
    assert_eq!(message.attachment_count(), 1);
    assert_eq!(message.size(), 4096);
    assert_eq!(
        message.time(),
        Timestamp::from_second(1_700_000_000).unwrap()
    );
    assert_eq!(message.order(), 42);
}

#[test]
fn fetched_message_has_body_and_attachments() {
    let wire = json!({
        "ID": "msg-1",
        "AddressID": "addr-1",
        "Subject": "With a file",
        "Body": "-----BEGIN PGP MESSAGE-----\nabc\n-----END PGP MESSAGE-----",
        "MIMEType": "text/html",
        "Header": "To: me@proton.me\r\n",
        "ParsedHeaders": {"To": "me@proton.me", "Received": ["one", "two"]},
        "Attachments": [{
            "ID": "att-1",
            "Name": "report.pdf",
            "Size": 90210,
            "MIMEType": "application/pdf",
            "Disposition": "attachment",
            "KeyPackets": "wcBMAw==",
            "Signature": "AQID",
            "Headers": {"content-id": "<cid-1>"}
        }]
    });

    let message: Message = read(&wire);
    let attachment = message.attachment(&"att-1".into()).unwrap();

    assert_eq!(message.mime_type(), &MimeType::Html);
    assert!(message.body().starts_with("-----BEGIN PGP MESSAGE-----"));
    assert_eq!(message.header(), "To: me@proton.me\r\n");
    assert_eq!(message.parsed_headers().len(), 2);
    assert_eq!(attachment.name(), "report.pdf");
    assert_eq!(attachment.size(), 90210);
    assert_eq!(attachment.content_id(), Some("cid-1"));
    assert_eq!(
        attachment.key_packet_bytes().unwrap(),
        [0xc1, 0xc0, 0x4c, 0x03]
    );
    assert_eq!(attachment.signature_bytes().unwrap(), Some(vec![1, 2, 3]));
}

#[test]
fn event_reports_changes() {
    let wire = json!({
        "Code": 1000,
        "EventID": "ev-2",
        "Refresh": 0,
        "More": 0,
        "Messages": [
            {"ID": "msg-1", "Action": 1,
             "Message": {"ID": "msg-1", "Subject": "New", "Unread": 1, "LabelIDs": ["0"]}},
            {"ID": "msg-2", "Action": 3,
             "Message": {"ID": "msg-2", "Subject": "Touched", "Unread": 0}},
            {"ID": "msg-3", "Action": 0}
        ],
        "Conversations": [
            {"ID": "conv-1", "Action": 2,
             "Conversation": {"ID": "conv-1", "NumMessages": 2, "NumUnread": 1}}
        ],
        "Labels": [
            {"ID": "lbl-1", "Action": 1,
             "Label": {"ID": "lbl-1", "Name": "Money", "Type": 3}}
        ],
        "Addresses": [
            {"ID": "addr-1", "Action": 2,
             "Address": {"ID": "addr-1", "Email": "me@proton.me", "Status": 1, "Type": 1}}
        ],
        "UsedSpace": 123_456
    });

    let event: Event = read(&wire);

    assert_eq!(event.id().as_str(), "ev-2");
    assert!(!event.needs_refresh());
    assert!(!event.has_more());
    assert_eq!(event.messages().len(), 3);
    assert_eq!(event.messages()[0].action(), EventAction::Create);
    assert_eq!(event.messages()[1].action(), EventAction::UpdateFlags);
    assert!(event.messages()[2].is_deleted());
    assert!(event.messages()[2].message().is_none());
    assert_eq!(
        event.conversations()[0]
            .conversation()
            .map(Conversation::messages),
        Some(2)
    );
    assert_eq!(event.labels()[0].label().map(Label::name), Some("Money"));
    assert_eq!(
        event.addresses()[0]
            .address()
            .map(rikka_proton_account::Address::email),
        Some("me@proton.me")
    );
    assert_eq!(event.used_space(), Some(123_456));
}
