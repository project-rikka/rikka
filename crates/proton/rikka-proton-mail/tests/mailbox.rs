use base64::prelude::{BASE64_STANDARD, Engine as _};
use jiff::Timestamp;
use rikka_proton_mail::{
    ConversationId, DirectAction, ImportMessage, LabelId, MessageFlags, MessageId, MimeMessage,
    PartBody,
};
use rikka_proton_pgp::{Pgp, PrivateKey};
use serde_json::{Value, json};

use crate::support::signed_in;
use rikka_test_proton::{ADDRESS_ID, Record, Request, json_response, new_key, not_found, response};

#[tokio::test(flavor = "multi_thread")]
async fn message_ids_page_after_last() {
    let all: Vec<String> = (0..5).map(|at| format!("id-{at}")).collect();
    let record = Record::default();
    let log = record.clone();

    let signed = signed_in(move |request: Request| {
        log.push(&request);
        if request.route() != ("GET", "/mail/v4/messages/ids") {
            return not_found();
        }
        let limit: usize = request
            .query("Limit")
            .unwrap_or_default()
            .parse()
            .unwrap_or(1000);
        let start = all
            .iter()
            .position(|id| *id == request.query("AfterID").unwrap_or_default())
            .map_or(0, |at| at + 1);
        let page: Vec<&String> = all.iter().skip(start).take(limit).collect();
        json_response(200, &json!({"Code": 1000, "IDs": page}))
    })
    .await;

    let walked = signed.client.message_ids().limit(2).all().await.unwrap();
    let tail = signed
        .client
        .message_ids()
        .starting_after(&MessageId::from("id-2"))
        .list()
        .await
        .unwrap();

    assert_eq!(walked.len(), 5);
    assert_eq!(walked[4], MessageId::from("id-4"));
    assert_eq!(tail, vec![MessageId::from("id-3"), MessageId::from("id-4")]);
    assert_eq!(record.matching("GET", "/mail/v4/messages/ids").len(), 5);
}

#[tokio::test(flavor = "multi_thread")]
async fn mailbox_actions_match_web_requests() {
    let record = Record::default();
    let log = record.clone();
    let signed = signed_in(move |request: Request| {
        log.push(&request);
        match request.route() {
            ("PUT", "/mail/v4/conversations/snooze" | "/mail/v4/conversations/unsnooze") => {
                json_response(
                    200,
                    &json!({"Code": 1001, "Responses": [{"ID": "c1", "Response": {"Code": 1000}}],
                        "UndoToken": {"Token": "undo-1", "ValidUntil": 99}}),
                )
            }
            ("POST", path) if path.starts_with("/mail/v4/messages/batch/") => {
                response(200, r#"{"Code":1000}"#)
            }
            ("DELETE", "/mail/v4/messages/empty") | ("POST", "/mail/v4/undoactions") => {
                response(200, r#"{"Code":1000}"#)
            }
            _ => not_found(),
        }
    })
    .await;
    let client = &signed.client;
    let conversations = [ConversationId::from("c1")];
    let inbox = [LabelId::from("0")];
    let archive = LabelId::from("6");

    let snoozed = client
        .conversation_batch(&conversations)
        .snooze(Timestamp::from_second(1_800_000_000).unwrap())
        .await
        .unwrap();
    client
        .conversation_batch(&conversations)
        .unsnooze()
        .await
        .unwrap();
    client.in_labels(&inbox).mark_read().await.unwrap();
    client.in_labels(&inbox).mark_unread().await.unwrap();
    client
        .in_labels(&inbox)
        .relabel(&[LabelId::from("label-a")], &[LabelId::from("label-b")])
        .await
        .unwrap();
    client.in_labels(&inbox).move_to(&archive).await.unwrap();
    client
        .empty_label(&LabelId::from("3"), Some(&ADDRESS_ID.into()))
        .await
        .unwrap();
    client.undo(&snoozed.undo_tokens()[0]).await.unwrap();

    assert!(snoozed.is_complete());
    let body = |method: &str, path: &str| record.matching(method, path)[0].json();
    let context = json!({"LabelIDs": ["0"]});

    assert_eq!(
        body("PUT", "/mail/v4/conversations/snooze"),
        json!({"IDs": ["c1"], "SnoozeTime": 1_800_000_000})
    );
    assert_eq!(
        body("PUT", "/mail/v4/conversations/unsnooze"),
        json!({"IDs": ["c1"]})
    );
    assert_eq!(
        body("POST", "/mail/v4/messages/batch/read"),
        json!({"SearchContext": context})
    );
    assert_eq!(
        body("POST", "/mail/v4/messages/batch/unread"),
        json!({"SearchContext": context})
    );
    assert_eq!(
        body("POST", "/mail/v4/messages/batch/label"),
        json!({"SearchContext": context, "AddLabelIDs": ["label-a"], "RemoveLabelIDs": ["label-b"]})
    );
    assert_eq!(
        body("POST", "/mail/v4/messages/batch/move"),
        json!({"SearchContext": context, "DestinationLabelID": "6"})
    );
    assert_eq!(
        record.matching("DELETE", "/mail/v4/messages/empty")[0].query,
        format!("LabelID=3&AddressID={ADDRESS_ID}")
    );
    assert_eq!(
        body("POST", "/mail/v4/undoactions"),
        json!({"Token": "undo-1"})
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn direct_send_request_shape() {
    let pgp = Pgp::new();
    let friend = new_key(&pgp, "friend@proton.test");
    let armored = friend.public_key().export_armored().unwrap();
    let record = Record::default();
    let log = record.clone();

    let signed = signed_in(move |request: Request| {
        log.push(&request);
        match request.route() {
            ("GET", "/core/v4/keys/all") if request.query.contains("friend") => json_response(
                200,
                &json!({"Code": 1000, "ProtonMX": true, "IsProton": 1,
                        "Address": {"Keys": [{"Flags": 3, "PublicKey": armored}]}}),
            ),
            ("GET", "/core/v4/keys/all") => json_response(
                200,
                &json!({"Code": 1000, "Address": {"Keys": []}, "ProtonMX": false, "IsProton": 0}),
            ),
            ("POST", "/mail/v4/messages/send/direct") => json_response(
                200,
                &json!({"Code": 1000, "DeliveryTime": 1_700_000_100,
                        "Sent": {"ID": "sent-1", "Subject": "Report", "Flags": 2}}),
            ),
            _ => not_found(),
        }
    })
    .await;

    let sent = signed
        .client
        .direct()
        .sender(&ADDRESS_ID.into())
        .subject("Report")
        .to("friend@proton.test")
        .cc("outsider@elsewhere.test")
        .body("numbers inside")
        .mime_type(rikka_proton_mail::MimeType::PlainText)
        .attach("first.txt", "text/plain", b"first file".to_vec())
        .attach_inline("logo.png", "image/png", "logo@here", b"png bytes".to_vec())
        .in_reply_to(&MessageId::from("parent-1"), DirectAction::Reply)
        .send()
        .await
        .unwrap();

    assert_eq!(
        sent.delivery_time(),
        Timestamp::from_second(1_700_000_100).unwrap()
    );
    assert_eq!(
        sent.sent().map(|message| message.id().as_str()),
        Some("sent-1")
    );

    let sent_body = record.matching("POST", "/mail/v4/messages/send/direct")[0].json();
    assert_eq!(sent_body["ParentID"], json!("parent-1"));
    assert_eq!(sent_body["Action"], json!(0));
    assert_eq!(sent_body["AutoSaveContacts"], json!(0));

    let copy = signed_copy(&pgp, &signed.address_key, &sent_body["Message"]["Body"]);
    assert_eq!(copy, b"numbers inside");

    let files = sent_body["Message"]["Attachments"].as_array().unwrap();
    assert_eq!(files[0]["Filename"], json!("first.txt"));
    assert_eq!(files[1]["ContentID"], json!("logo@here"));
    for file in files {
        let contents = BASE64_STANDARD
            .decode(file["Contents"].as_str().unwrap())
            .unwrap();
        assert!(!contents.is_empty());
    }

    let package = &sent_body["Packages"][0];
    let internal = &package["Addresses"]["friend@proton.test"];
    let packets = internal["AttachmentKeyPackets"].as_array().unwrap();
    let exposed = package["AttachmentKeys"].as_array().unwrap();
    assert_eq!(internal["Type"], json!(1));
    assert_eq!(
        package["Addresses"]["outsider@elsewhere.test"]["Type"],
        json!(4)
    );
    assert_eq!(packets.len(), 2);
    assert_eq!(exposed.len(), 2);

    for (packet, key) in packets.iter().zip(exposed) {
        let packet = BASE64_STANDARD.decode(packet.as_str().unwrap()).unwrap();
        let session_key = pgp
            .decrypt()
            .with_key(friend.clone())
            .session_key(&packet)
            .unwrap();
        assert_eq!(
            BASE64_STANDARD.encode(session_key.as_bytes()),
            key["Key"].as_str().unwrap()
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn import_encrypts_messages() {
    let record = Record::default();
    let log = record.clone();
    let signed = signed_in(move |request: Request| {
        log.push(&request);
        match request.route() {
            ("POST", "/mail/v4/messages/import") => json_response(
                200,
                &json!({"Code": 1001, "Responses": [
                    {"Name": "1", "Response": {"Code": 2001, "Error": "refused"}},
                    {"Name": "0", "Response": {"Code": 1000, "MessageID": "new-0"}}
                ]}),
            ),
            _ => not_found(),
        }
    })
    .await;

    let mixed = "Subject: Hello\r\nFrom: a@example.test\r\n\
        Content-Type: multipart/mixed; boundary=\"m\"\r\n\r\n\
        --m\r\nContent-Type: text/plain; charset=iso-8859-1\r\n\
        Content-Transfer-Encoding: quoted-printable\r\n\r\nCaf=E9\r\n\
        --m\r\nContent-Type: application/octet-stream; name=\"data.bin\"\r\n\
        Content-Transfer-Encoding: base64\r\n\r\nAAEC\r\n--m--\r\n";
    let signed_literal = "Subject: Signed\r\nFrom: b@example.test\r\n\
        Content-Type: multipart/signed; boundary=\"s\"; protocol=\"application/pgp-signature\"\r\n\r\n\
        --s\r\nContent-Type: text/plain\r\n\r\nsigned\r\n--s\r\n\
        Content-Type: application/pgp-signature\r\n\r\nnot really\r\n--s--\r\n";

    let address = ADDRESS_ID.into();
    let outcomes = signed
        .client
        .import()
        .message(
            ImportMessage::new(mixed, &address)
                .label(&LabelId::from("0"))
                .unread(true),
        )
        .message(
            ImportMessage::new(signed_literal, &address)
                .flags(MessageFlags::RECEIVED | MessageFlags::IMPORTED),
        )
        .run()
        .await
        .unwrap();

    assert!(outcomes[0].is_ok());
    assert_eq!(outcomes[0].message_id(), Some(&MessageId::from("new-0")));
    assert!(!outcomes[1].is_ok());
    assert_eq!(outcomes[1].error(), Some("refused"));

    let form = record.matching("POST", "/mail/v4/messages/import")[0].form();
    let field = |name: &str| {
        form.iter()
            .find(|(field_name, _)| field_name == name)
            .map(|(_, value)| value.clone())
            .unwrap()
    };

    let metadata: Value = serde_json::from_slice(&field("Metadata")).unwrap();
    assert_eq!(
        metadata["0"],
        json!({"AddressID": ADDRESS_ID, "LabelIDs": ["0"], "Unread": 1, "Flags": 1})
    );
    assert_eq!(metadata["1"]["Flags"], json!(513));

    let reader = &signed.address_key;
    let pgp = Pgp::new();

    let parted = MimeMessage::parse(field("0"));
    assert_eq!(parted.headers().get("subject").as_deref(), Some("Hello"));
    let [text, data] = parted.root().children() else {
        panic!("expected multipart body");
    };
    assert_eq!(text.content_type().charset(), Some("utf-8"));
    assert!(!text.headers().contains("content-transfer-encoding"));
    let opened_text = pgp
        .decrypt()
        .with_key(reader.clone())
        .message(text.data().unwrap())
        .unwrap();
    assert_eq!(opened_text.data(), "Café".as_bytes());
    let PartBody::Leaf(sealed_data) = data.body() else {
        panic!("expected leaf part");
    };
    let opened_data = pgp
        .decrypt()
        .with_key(reader.clone())
        .message(sealed_data)
        .unwrap();
    assert_eq!(opened_data.data(), &[0, 1, 2]);

    let whole = MimeMessage::parse(field("1"));
    assert!(whole.root().content_type().is("multipart/encrypted"));
    assert_eq!(whole.headers().get("subject").as_deref(), Some("Signed"));
    let payload = whole.root().children()[1].data().unwrap();
    let opened_whole = pgp
        .decrypt()
        .with_key(reader.clone())
        .message(payload)
        .unwrap();
    assert_eq!(opened_whole.data(), signed_literal.as_bytes());
}

fn signed_copy(pgp: &Pgp, reader: &PrivateKey, body: &Value) -> Vec<u8> {
    pgp.decrypt()
        .with_key(reader.clone())
        .message(body.as_str().unwrap())
        .unwrap()
        .into_data()
}
