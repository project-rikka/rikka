use std::sync::{Arc, Mutex};

use crate::support::signed_in;
use base64::prelude::{BASE64_STANDARD, Engine as _};
use rikka_proton_mail::{AttachmentId, MessageId, MimeType, PackageKind};
use rikka_proton_pgp::{Pgp, PrivateKey, SessionKey};
use rikka_test_proton::{ADDRESS_ID, EMAIL, Request, json_response, new_key, not_found};
use serde_json::{Value, json};

const BODY: &str = "<p>the body of the message</p>";

fn decrypt_session_key(pgp: &Pgp, key: &PrivateKey, packet: &str) -> SessionKey {
    let packet = BASE64_STANDARD.decode(packet).unwrap();

    pgp.decrypt()
        .with_key(key.clone())
        .session_key(packet)
        .unwrap()
}

fn decrypt_body(pgp: &Pgp, session_key: SessionKey, body: &str) -> Vec<u8> {
    let data = BASE64_STANDARD.decode(body).unwrap();

    pgp.decrypt()
        .with_session_key(session_key)
        .message(data)
        .unwrap()
        .into_data()
}

#[tokio::test(flavor = "multi_thread")]
async fn draft_body_encrypted_and_signed_by_sender() {
    let signed = signed_in(|request: Request| match request.route() {
        ("POST", "/mail/v4/messages") => {
            let mut message = request.json()["Message"].clone();
            message["ID"] = Value::String("msg-1".to_owned());
            message["AddressID"] = Value::String(ADDRESS_ID.to_owned());

            json_response(200, &json!({"Code": 1000, "Message": message}))
        }
        _ => not_found(),
    })
    .await;

    let created = signed
        .client
        .draft()
        .sender(&ADDRESS_ID.into())
        .subject("Hello")
        .to("who@proton.test")
        .body(BODY)
        .mime_type(MimeType::Html)
        .create()
        .await
        .unwrap();

    assert_eq!(created.sender().address(), EMAIL);
    assert_eq!(created.sender().name(), "Tester");
    assert!(created.body().starts_with("-----BEGIN PGP MESSAGE-----"));

    let decrypted = signed
        .client
        .decrypt_message_with(&created, &[signed.address_key.public_key()])
        .unwrap();

    assert_eq!(decrypted.text(), BODY);
    assert!(decrypted.is_verified(), "draft not signed by its address");
}

#[tokio::test(flavor = "multi_thread")]
async fn send_groups_packages_by_scheme() {
    let signed = signed_in(|_: Request| not_found()).await;
    let pgp = Pgp::new();
    let recipient = new_key(&pgp, "inside@proton.test");
    let attachment = AttachmentId::from("att-1");
    let attachment_key = SessionKey::generate();

    let packages = signed
        .client
        .send(&MessageId::from("msg-1"))
        .sender(&ADDRESS_ID.into())
        .body(BODY)
        .mime_type(MimeType::Html)
        .attachment_key(&attachment, attachment_key.clone())
        .to_internal("inside@proton.test", recipient.public_key())
        .to_cleartext("outside@other.test")
        .packages()
        .unwrap();

    assert_eq!(packages.len(), 1, "expected one package");

    let package = &packages[0];
    let kinds = package.kinds();

    assert_eq!(package.mime_type(), &MimeType::Html);
    assert_eq!(kinds.bits(), 5);
    assert!(kinds.contains(PackageKind::Internal));
    assert!(kinds.contains(PackageKind::Cleartext));
    assert_eq!(package.addresses().len(), 2);

    let inside = package.address("inside@proton.test").unwrap();
    let outside = package.address("outside@other.test").unwrap();

    assert_eq!(inside.kind(), Some(PackageKind::Internal));
    assert_eq!(outside.kind(), Some(PackageKind::Cleartext));
    assert!(outside.body_key_packet().is_none());
    assert!(outside.attachment_key_packets().is_empty());

    let session_key = decrypt_session_key(&pgp, &recipient, inside.body_key_packet().unwrap());

    assert_eq!(
        decrypt_body(&pgp, session_key, package.body()),
        BODY.as_bytes(),
        "recipient key does not decrypt body"
    );

    let exposed = package.body_key().unwrap();
    let bytes = BASE64_STANDARD.decode(exposed.key()).unwrap();

    assert_eq!(exposed.algorithm(), "aes256");
    assert_eq!(
        decrypt_body(&pgp, SessionKey::from_bytes(bytes).unwrap(), package.body()),
        BODY.as_bytes(),
        "exposed key does not decrypt body"
    );

    let packet = inside.attachment_key_packets().get(&attachment).unwrap();

    assert_eq!(
        decrypt_session_key(&pgp, &recipient, packet).as_bytes(),
        attachment_key.as_bytes()
    );
    assert_eq!(
        package
            .attachment_keys()
            .get(&attachment)
            .map(|key| BASE64_STANDARD.decode(key.key()).unwrap()),
        Some(attachment_key.as_bytes().to_vec()),
        "cleartext recipient missing attachment key"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_send_without_recipient_or_mime() {
    let signed = signed_in(|_: Request| not_found()).await;
    let pgp = Pgp::new();
    let recipient = new_key(&pgp, "pgp@other.test");

    let bare = signed
        .client
        .send(&MessageId::from("msg-1"))
        .sender(&ADDRESS_ID.into())
        .body(BODY)
        .packages()
        .expect_err("a message needs a recipient");

    assert!(matches!(bare, rikka_proton_mail::Error::NoRecipient));

    let missing_body = signed
        .client
        .send(&MessageId::from("msg-1"))
        .sender(&ADDRESS_ID.into())
        .body(BODY)
        .to_pgp_mime("pgp@other.test", recipient.public_key())
        .packages()
        .expect_err("a mime package needs a mime body");

    assert!(matches!(
        missing_body,
        rikka_proton_mail::Error::MissingMimeBody
    ));

    let unresolved = signed
        .client
        .send(&MessageId::from("msg-1"))
        .sender(&ADDRESS_ID.into())
        .body(BODY)
        .to("stranger@other.test")
        .packages()
        .expect_err("a recipient that was never looked up cannot be packaged");

    assert!(matches!(
        unresolved,
        rikka_proton_mail::Error::UnresolvedRecipient(_)
    ));

    let inline = signed
        .client
        .send(&MessageId::from("msg-1"))
        .sender(&ADDRESS_ID.into())
        .body(BODY)
        .mime_type(MimeType::Html)
        .to_pgp_inline("pgp@other.test", recipient.public_key())
        .packages()
        .unwrap();

    assert_eq!(inline.len(), 1);
    assert_eq!(inline[0].mime_type(), &MimeType::PlainText);
    let packet = BASE64_STANDARD
        .decode(
            inline[0]
                .address("pgp@other.test")
                .and_then(|address| address.body_key_packet())
                .unwrap(),
        )
        .unwrap();
    let mut sealed = packet;
    sealed.extend(BASE64_STANDARD.decode(inline[0].body()).unwrap());
    let decrypted = pgp
        .decrypt()
        .with_key(recipient.clone())
        .message(&sealed)
        .unwrap();
    assert_eq!(decrypted.data(), b"the body of the message");
}

#[tokio::test(flavor = "multi_thread")]
async fn attachment_opens_with_key_packet() {
    let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let served = captured.clone();

    let signed = signed_in(move |request: Request| match request.route() {
        ("GET", "/mail/v4/attachments/att-1") => {
            rikka_test_proton::binary_response(200, served.lock().unwrap().clone())
        }
        _ => not_found(),
    })
    .await;

    let pgp = Pgp::new();
    let key = signed.address_key.clone();
    let split = pgp
        .encrypt()
        .to(key.public_key())
        .split(b"file data")
        .unwrap();
    let signature = pgp
        .sign()
        .with_key(key.clone())
        .detached(b"file data")
        .unwrap();

    captured
        .lock()
        .unwrap()
        .extend_from_slice(split.data_packet());

    let attachment: rikka_proton_mail::Attachment = serde_json::from_value(json!({
        "ID": "att-1",
        "Name": "note.txt",
        "Size": 9,
        "MIMEType": "text/plain",
        "Disposition": "attachment",
        "KeyPackets": BASE64_STANDARD.encode(split.key_packets()),
        "Signature": BASE64_STANDARD.encode(&signature)
    }))
    .unwrap();

    let decrypted = signed
        .client
        .attachment(&attachment, &[key.public_key()])
        .await
        .unwrap();

    assert_eq!(decrypted.bytes(), b"file data");
    assert_eq!(decrypted.name(), "note.txt");
    assert_eq!(decrypted.mime_type(), "text/plain");
    assert!(decrypted.is_verified(), "detached signature not verified");

    let session_key = signed.client.attachment_session_key(&attachment).unwrap();

    assert_eq!(session_key.as_bytes().len(), 32);
}
