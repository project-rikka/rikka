use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::support::signed_in;
use base64::prelude::{BASE64_STANDARD, Engine as _};
use rikka_proton_mail::{AttachmentId, MessageId, MimeType};
use rikka_proton_pgp::Pgp;
use rikka_test_proton::{
    ADDRESS_ID, Record, Request, SIGNED_MODULUS, json_response, not_found, response,
};
use serde_json::json;

type Form = BTreeMap<String, Vec<u8>>;
type StoredAccount = Arc<Mutex<Option<Form>>>;

fn form_parts(request: &Request) -> Form {
    request.form().into_iter().collect()
}
#[tokio::test(flavor = "multi_thread")]
async fn attachment_upload_round_trips() {
    let carried: StoredAccount = Arc::new(Mutex::new(None));
    let captured = carried.clone();

    let signed = signed_in(move |request: Request| match request.route() {
        ("POST", "/mail/v4/attachments") => {
            let parts = form_parts(&request);
            let packets = BASE64_STANDARD.encode(parts.get("KeyPackets").unwrap());

            *captured.lock().unwrap() = Some(parts);

            json_response(
                200,
                &json!({"Code": 1000, "Attachment": {
                    "ID": "att-1",
                    "Name": "note.txt",
                    "Size": 9,
                    "MIMEType": "text/plain",
                    "Disposition": "inline",
                    "ContentID": "cid-1",
                    "KeyPackets": packets
                }}),
            )
        }
        ("DELETE", "/mail/v4/attachments/att-1") => response(200, r#"{"Code":1000}"#),
        _ => not_found(),
    })
    .await;

    let attachment = signed
        .client
        .upload(&MessageId::from("msg-1"), "note.txt", b"file data".to_vec())
        .sender(&ADDRESS_ID.into())
        .mime_type("text/plain")
        .inline("cid-1")
        .upload()
        .await
        .unwrap();

    assert_eq!(attachment.id(), &AttachmentId::from("att-1"));
    assert_eq!(attachment.content_id(), Some("cid-1"));

    let form = carried.lock().unwrap().clone().unwrap();

    assert_eq!(
        form.get("MessageID").map(Vec::as_slice),
        Some(&b"msg-1"[..])
    );
    assert_eq!(
        form.get("Filename").map(Vec::as_slice),
        Some(&b"note.txt"[..])
    );
    assert_eq!(
        form.get("MIMEType").map(Vec::as_slice),
        Some(&b"text/plain"[..])
    );
    assert_eq!(
        form.get("Disposition").map(Vec::as_slice),
        Some(&b"inline"[..])
    );
    assert_eq!(
        form.get("ContentID").map(Vec::as_slice),
        Some(&b"cid-1"[..])
    );

    let pgp = Pgp::new();
    let session_key = pgp
        .decrypt()
        .with_key(signed.address_key.clone())
        .session_key(form.get("KeyPackets").unwrap())
        .unwrap();
    let decrypted = pgp
        .decrypt()
        .with_session_key(session_key)
        .message(form.get("DataPacket").unwrap())
        .unwrap();

    assert_eq!(decrypted.data(), b"file data");
    assert!(
        pgp.verify()
            .with_key(signed.address_key.public_key())
            .detached(b"file data", form.get("Signature").unwrap())
            .is_valid(),
        "attachment not signed by sender"
    );

    signed
        .client
        .delete_attachment(&AttachmentId::from("att-1"))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn send_resolves_recipients() {
    let record = Record::default();
    let log = record.clone();
    let pgp = Pgp::new();
    let inside = rikka_test_proton::new_key(&pgp, "inside@proton.test");
    let armored = inside.public_key().export_armored().unwrap();

    let signed = signed_in(move |request: Request| {
        log.push(&request);

        match request.route() {
            ("GET", "/core/v4/keys/all") if request.query.contains("inside") => json_response(
                200,
                &json!({"Code": 1000, "ProtonMX": true, "IsProton": 1,
                        "Address": {"Keys": [{"Flags": 3, "PublicKey": armored}]}}),
            ),
            ("GET", "/core/v4/keys/all") => json_response(
                200,
                &json!({"Code": 1000, "Address": {"Keys": []}, "ProtonMX": false, "IsProton": 0}),
            ),
            ("POST", "/mail/v4/messages/msg-1") => json_response(
                200,
                &json!({"Code": 1000, "Sent": {"ID": "msg-1", "Subject": "Hello", "Flags": 2}}),
            ),
            _ => not_found(),
        }
    })
    .await;

    let sent = signed
        .client
        .send(&MessageId::from("msg-1"))
        .sender(&ADDRESS_ID.into())
        .body("the body")
        .mime_type(MimeType::PlainText)
        .to("inside@proton.test")
        .to("outside@other.test")
        .auto_save_contacts(true)
        .expires_in(std::time::Duration::from_secs(3600))
        .send()
        .await
        .unwrap();

    assert_eq!(sent.id().as_str(), "msg-1");

    let request = record.requests();
    let sent_body = request
        .iter()
        .find(|request| request.route() == ("POST", "/mail/v4/messages/msg-1"))
        .unwrap()
        .json();

    assert_eq!(
        request
            .iter()
            .filter(|request| request.route() == ("GET", "/core/v4/keys/all"))
            .count(),
        2,
        "every recipient without a scheme is looked up"
    );
    assert_eq!(sent_body["AutoSaveContacts"], json!(1));
    assert_eq!(sent_body["ExpiresIn"], json!(3600));
    assert_eq!(sent_body["Packages"].as_array().map(Vec::len), Some(1));

    let package = &sent_body["Packages"][0];
    let addresses = &package["Addresses"];

    assert_eq!(package["MIMEType"], json!("text/plain"));
    assert_eq!(package["Type"], json!(5));
    assert_eq!(addresses["inside@proton.test"]["Type"], json!(1));
    assert_eq!(addresses["inside@proton.test"]["Signature"], json!(1));
    assert_eq!(addresses["outside@other.test"]["Type"], json!(4));
    assert_eq!(addresses["outside@other.test"]["Signature"], json!(0));
    assert!(addresses["outside@other.test"]["BodyKeyPacket"].is_null());
    assert_eq!(package["BodyKey"]["Algorithm"], json!("aes256"));

    let packet = BASE64_STANDARD
        .decode(
            addresses["inside@proton.test"]["BodyKeyPacket"]
                .as_str()
                .unwrap_or_default(),
        )
        .unwrap();
    let session_key = pgp
        .decrypt()
        .with_key(inside.clone())
        .session_key(packet)
        .unwrap();
    let body = BASE64_STANDARD
        .decode(package["Body"].as_str().unwrap_or_default())
        .unwrap();

    assert_eq!(
        pgp.decrypt()
            .with_session_key(session_key)
            .message(body)
            .unwrap()
            .data(),
        b"the body"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn password_recipient_gets_srp_verifier() {
    let record = Record::default();
    let log = record.clone();
    let signed = signed_in(move |request: Request| {
        log.push(&request);

        match request.route() {
            ("GET", "/auth/v4/modulus") => json_response(
                200,
                &json!({"Code": 1000, "ModulusID": "mod-1", "Modulus": SIGNED_MODULUS}),
            ),
            ("POST", "/mail/v4/messages/msg-1") => {
                json_response(200, &json!({"Code": 1000, "Sent": {"ID": "msg-1"}}))
            }
            _ => not_found(),
        }
    })
    .await;

    signed
        .client
        .send(&MessageId::from("msg-1"))
        .sender(&ADDRESS_ID.into())
        .body("the body")
        .mime_type(MimeType::PlainText)
        .password_hint("the usual one")
        .to_password("outside@other.test", "hunter2")
        .send()
        .await
        .unwrap();

    let request = record.requests();
    let sent_body = request
        .iter()
        .find(|request| request.route() == ("POST", "/mail/v4/messages/msg-1"))
        .unwrap()
        .json();
    let package = &sent_body["Packages"][0];
    let address = &package["Addresses"]["outside@other.test"];

    assert_eq!(package["Type"], json!(2));
    assert!(package["BodyKey"].is_null(), "body is not encrypted");
    assert_eq!(address["Type"], json!(2));
    assert_eq!(address["Signature"], json!(1));
    assert_eq!(address["PasswordHint"], json!("the usual one"));
    assert_eq!(address["Auth"]["ModulusID"], json!("mod-1"));
    assert_eq!(address["Auth"]["Version"], json!(4));
    assert!(address["Auth"]["Salt"].is_string());
    assert!(address["Auth"]["Verifier"].is_string());

    let pgp = Pgp::new();
    let packet = BASE64_STANDARD
        .decode(address["BodyKeyPacket"].as_str().unwrap_or_default())
        .unwrap();
    let session_key = pgp
        .decrypt()
        .with_password("hunter2")
        .session_key(packet)
        .unwrap();
    let body = BASE64_STANDARD
        .decode(package["Body"].as_str().unwrap_or_default())
        .unwrap();
    let decrypted = pgp
        .decrypt()
        .with_session_key(session_key)
        .message(body)
        .unwrap();

    assert_eq!(decrypted.data(), b"the body");

    let token = pgp
        .decrypt()
        .with_password("hunter2")
        .message(address["EncToken"].as_str().unwrap_or_default())
        .unwrap();

    assert_eq!(
        String::from_utf8_lossy(token.data()),
        address["Token"].as_str().unwrap_or_default(),
        "sealed token does not match plaintext token"
    );
}
