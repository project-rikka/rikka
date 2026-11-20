use std::sync::{Arc, Mutex};

use base64::prelude::{BASE64_STANDARD, Engine as _};
use rikka_proton_mail::{DirectSend, MessageId, MimeMessage, MimeType};
use rikka_proton_pgp::{Pgp, PrivateKey, PublicKey, SessionKey};
use serde_json::{Value, json};

use crate::support::{SignedIn, signed_in_with};
use rikka_test_proton::{
    ADDRESS_ID, EMAIL, Request, ResponseFuture, binary_response, json_response, new_key, not_found,
};

const INTERNAL: &str = "friend@proton.test";
const PLAIN: &str = "plain@elsewhere.test";
const WKD: &str = "wkd@elsewhere.test";
const BODY: &str = "<p>quarterly <b>numbers</b></p>";

struct Keys {
    friend: PrivateKey,
    wkd: PrivateKey,
}

impl Keys {
    fn new(pgp: &Pgp) -> Keys {
        Keys {
            friend: new_key(pgp, INTERNAL),
            wkd: new_key(pgp, WKD),
        }
    }
}

fn armored(key: &PrivateKey) -> String {
    key.public_key().export_armored().unwrap()
}

fn published(request: &Request, keys: &Keys) -> Option<ResponseFuture> {
    let routed = match request.route() {
        ("GET", "/core/v4/keys/all") if request.query.contains("friend") => json!({
            "Code": 1000, "ProtonMX": true, "IsProton": 1,
            "Address": {"Keys": [{"Flags": 3, "PublicKey": armored(&keys.friend)}]}
        }),
        ("GET", "/core/v4/keys/all") if request.query.contains("wkd") => json!({
            "Code": 1000, "ProtonMX": false, "IsProton": 0,
            "Address": {"Keys": []},
            "Unverified": {"Keys": [{"Flags": 3, "Source": 1, "PublicKey": armored(&keys.wkd)}]}
        }),
        ("GET", "/core/v4/keys/all") => json!({
            "Code": 1000, "ProtonMX": false, "IsProton": 0, "Address": {"Keys": []}
        }),
        ("GET", "/contacts/v4/contacts/emails") => {
            json!({"Code": 1000, "ContactEmails": [], "Total": 0})
        }
        _ => return None,
    };
    Some(json_response(200, &routed))
}

async fn direct(
    settings: Value,
    configure: impl FnOnce(DirectSend) -> DirectSend,
) -> (Value, SignedIn) {
    let pgp = Pgp::new();
    let keys = Arc::new(Keys::new(&pgp));
    let sent: Arc<Mutex<Option<Value>>> = Arc::default();
    let sent_handle = Arc::clone(&sent);
    let known = Arc::clone(&keys);

    let signed = signed_in_with(settings, move |request: Request| {
        if let Some(found) = published(&request, &known) {
            return found;
        }
        match request.route() {
            ("POST", "/mail/v4/messages/send/direct") => {
                *sent_handle.lock().unwrap() = Some(request.json());
                json_response(200, &json!({"Code": 1000, "DeliveryTime": 1}))
            }
            _ => not_found(),
        }
    })
    .await;

    let sending = configure(
        signed
            .client
            .direct()
            .sender(&ADDRESS_ID.into())
            .subject("Report")
            .body(BODY),
    );
    sending.send().await.unwrap();

    let sent_body = sent.lock().unwrap().clone().unwrap();
    (sent_body, signed)
}

fn package_shape(sent_body: &Value, email: &str) -> (String, i64, i64) {
    let package = sent_body["Packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["Addresses"].get(email).is_some())
        .unwrap();
    let address = &package["Addresses"][email];
    (
        package["MIMEType"].as_str().unwrap().to_owned(),
        address["Type"].as_i64().unwrap(),
        address["Signature"].as_i64().unwrap(),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn recipients_follow_mail_settings() {
    type Configure = fn(DirectSend) -> DirectSend;
    type Expected = (&'static str, i64, i64);
    let html: Configure = |sending| sending.mime_type(MimeType::Html);
    let forced: Configure = |sending| sending.mime_type(MimeType::Html).sign(true);
    let unsigned: Configure = |sending| sending.mime_type(MimeType::Html).sign(false);
    let unchanged: Configure = |sending| sending;

    let cases: Vec<(&str, Value, Configure, &str, Expected)> = vec![
        (
            "clear by default",
            json!({"Sign": 0}),
            html,
            PLAIN,
            ("text/html", 4, 0),
        ),
        (
            "signed clear MIME",
            json!({"Sign": 1, "PGPScheme": 16}),
            html,
            PLAIN,
            ("multipart/mixed", 32, 1),
        ),
        (
            "signed PGP inline",
            json!({"Sign": 1, "PGPScheme": 8}),
            html,
            PLAIN,
            ("text/plain", 4, 1),
        ),
        (
            "signing forced for this send",
            json!({"Sign": 0, "PGPScheme": 16}),
            forced,
            PLAIN,
            ("multipart/mixed", 32, 1),
        ),
        (
            "signing turned off for this send",
            json!({"Sign": 1, "PGPScheme": 16}),
            unsigned,
            PLAIN,
            ("text/html", 4, 0),
        ),
        (
            "internal keeps the message type",
            json!({"Sign": 1, "PGPScheme": 16}),
            html,
            INTERNAL,
            ("text/html", 1, 1),
        ),
        (
            "internal under signed inline goes plain",
            json!({"Sign": 1, "PGPScheme": 8}),
            html,
            INTERNAL,
            ("text/plain", 1, 1),
        ),
        (
            "an external key over PGP/MIME",
            json!({"Sign": 0, "PGPScheme": 16}),
            html,
            WKD,
            ("multipart/mixed", 16, 0),
        ),
        (
            "an external key over PGP inline",
            json!({"Sign": 0, "PGPScheme": 8}),
            html,
            WKD,
            ("text/plain", 8, 1),
        ),
        (
            "the draft type setting",
            json!({"Sign": 0, "DraftMIMEType": "text/plain"}),
            unchanged,
            PLAIN,
            ("text/plain", 4, 0),
        ),
    ];

    for (name, settings, configure, email, (mime_type, kind, signature)) in cases {
        let (sent_body, _) = direct(settings, |sending| configure(sending.to(email))).await;
        assert_eq!(
            package_shape(&sent_body, email),
            (mime_type.to_owned(), kind, signature),
            "{name}"
        );
        if name == "the draft type setting" {
            assert_eq!(sent_body["Message"]["MIMEType"], "text/plain", "{name}");
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn signed_clear_mime_has_both_parts() {
    let (sent_body, signed) = direct(json!({"Sign": 1, "PGPScheme": 16}), |sending| {
        sending.to(PLAIN).mime_type(MimeType::Html).attach(
            "note.txt",
            "text/plain",
            b"see the numbers".to_vec(),
        )
    })
    .await;

    let package = &sent_body["Packages"][0];
    assert!(package.get("AttachmentKeys").is_none());
    let decrypted = decrypt_with_body_key(package, &signed.address_key.public_key());
    assert!(decrypted.1, "MIME body not signed by sender");

    let content = MimeMessage::parse(&decrypted.0).content();
    assert_eq!(content.html(), Some(BODY));
    assert_eq!(content.text(), "quarterly numbers");
    assert_eq!(content.attachments()[0].name(), "note.txt");
    assert_eq!(content.attachments()[0].bytes(), b"see the numbers");
    assert!(String::from_utf8_lossy(&decrypted.0).ends_with("\r\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn attaches_sender_key_when_requested() {
    let fingerprint = |signed: &SignedIn| {
        let full = signed.address_key.fingerprint();
        full.as_str()[..8].to_ascii_uppercase()
    };

    let cases: [(Value, Option<bool>, bool); 3] = [
        (json!({"AttachPublicKey": 1}), None, true),
        (json!({"AttachPublicKey": 1}), Some(false), false),
        (json!({"AttachPublicKey": 0}), Some(true), true),
    ];

    for (settings, choice, attached) in cases {
        let (sent_body, signed) = direct(settings, |sending| {
            let sending = sending.to(PLAIN);
            match choice {
                Some(choice) => sending.attach_public_key(choice),
                None => sending,
            }
        })
        .await;

        let names: Vec<&str> = sent_body["Message"]["Attachments"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|file| file["Filename"].as_str())
            .collect();
        let expected = format!("publickey - {EMAIL} - 0x{}.asc", fingerprint(&signed));
        assert_eq!(names.contains(&expected.as_str()), attached, "{names:?}");
        if attached {
            assert_eq!(
                sent_body["Message"]["Attachments"][0]["MIMEType"],
                "application/pgp-keys"
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn draft_send_builds_signed_mime() {
    let pgp = Pgp::new();
    let keys = Arc::new(Keys::new(&pgp));
    let known = Arc::clone(&keys);
    let reader: Arc<std::sync::OnceLock<PublicKey>> = Arc::default();
    let sealing = Arc::clone(&reader);
    let posted: Arc<Mutex<Vec<Request>>> = Arc::default();
    let log = Arc::clone(&posted);
    let file: Arc<std::sync::OnceLock<(Vec<u8>, Vec<u8>)>> = Arc::default();

    let signed = signed_in_with(
        json!({"Sign": 1, "PGPScheme": 16, "AttachPublicKey": 1}),
        move |request: Request| {
            if let Some(found) = published(&request, &known) {
                return found;
            }
            let reader = sealing.get().unwrap().clone();
            let (packets, data) = file.get_or_init(|| {
                let (packets, data, _) = Pgp::new()
                    .encrypt()
                    .to(reader)
                    .split(b"draft file")
                    .unwrap()
                    .into_parts();
                (packets, data)
            });
            match request.route() {
                ("GET", "/mail/v4/messages/msg-1") => json_response(
                    200,
                    &json!({"Code": 1000, "Message": {
                        "ID": "msg-1", "AddressID": ADDRESS_ID, "Flags": 0,
                        "Attachments": [{
                            "ID": "att-1", "Name": "draft.txt", "MIMEType": "text/plain",
                            "KeyPackets": BASE64_STANDARD.encode(packets)
                        }]
                    }}),
                ),
                ("GET", "/mail/v4/attachments/att-1") => binary_response(200, data.clone()),
                ("POST", "/mail/v4/attachments") => {
                    let packets = request
                        .form()
                        .into_iter()
                        .find(|(name, _)| name == "KeyPackets")
                        .map(|(_, value)| BASE64_STANDARD.encode(value))
                        .unwrap();
                    log.lock().unwrap().push(request);
                    json_response(
                        200,
                        &json!({"Code": 1000, "Attachment": {
                            "ID": "att-key", "Name": "key", "MIMEType": "application/pgp-keys",
                            "KeyPackets": packets
                        }}),
                    )
                }
                ("POST", "/mail/v4/messages/msg-1") => {
                    log.lock().unwrap().push(request);
                    json_response(
                        200,
                        &json!({"Code": 1000, "Sent": {"ID": "msg-1", "Flags": 2}}),
                    )
                }
                _ => not_found(),
            }
        },
    )
    .await;
    reader.set(signed.address_key.public_key()).unwrap();

    signed
        .client
        .send(&MessageId::from("msg-1"))
        .sender(&ADDRESS_ID.into())
        .body(BODY)
        .mime_type(MimeType::Html)
        .to(PLAIN)
        .send()
        .await
        .unwrap();

    let posted = posted.lock().unwrap();
    let upload = posted
        .iter()
        .find(|request| request.path == "/mail/v4/attachments")
        .unwrap()
        .form();
    let filename = upload
        .iter()
        .find(|(name, _)| name == "Filename")
        .map(|(_, value)| String::from_utf8_lossy(value).into_owned())
        .unwrap();
    assert!(filename.starts_with(&format!("publickey - {EMAIL} - 0x")));

    let sent = posted
        .iter()
        .find(|request| request.path == "/mail/v4/messages/msg-1")
        .unwrap()
        .json();
    let package = &sent["Packages"][0];
    assert_eq!(package["MIMEType"], "multipart/mixed");
    assert_eq!(package["Addresses"][PLAIN]["Type"], 32);

    let decrypted = decrypt_with_body_key(package, &signed.address_key.public_key());
    assert!(decrypted.1);
    let content = MimeMessage::parse(&decrypted.0).content();
    let names: Vec<&str> = content
        .attachments()
        .iter()
        .map(|file| file.name())
        .collect();
    assert_eq!(names, vec!["draft.txt", filename.as_str()]);
    assert_eq!(content.attachments()[0].bytes(), b"draft file");
}

fn decrypt_with_body_key(package: &Value, signer: &PublicKey) -> (Vec<u8>, bool) {
    let key = BASE64_STANDARD
        .decode(package["BodyKey"]["Key"].as_str().unwrap())
        .unwrap();
    let data = BASE64_STANDARD
        .decode(package["Body"].as_str().unwrap())
        .unwrap();
    let decrypted = Pgp::new()
        .decrypt()
        .with_session_key(SessionKey::from_bytes(key).unwrap())
        .verified_by(signer.clone())
        .message(data)
        .unwrap();
    let signed = decrypted.signature().is_valid();
    (decrypted.into_data(), signed)
}
