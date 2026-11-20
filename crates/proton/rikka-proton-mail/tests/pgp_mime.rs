use rikka_proton_mail::{Message, MimeType};
use rikka_proton_pgp::{Pgp, PrivateKey, PublicKey};
use serde_json::json;

use crate::support::signed_in;
use rikka_test_proton::{ADDRESS_ID, Request, new_key, not_found};

const SENDER: &str = "writer@elsewhere.test";
const SIGNED_PART: &str = "Content-Type: text/plain; charset=utf-8\r\n\r\nsigned words";

fn encrypted_message(pgp: &Pgp, reader: &PrivateKey, mime: &str) -> Message {
    let body = pgp
        .encrypt()
        .to(reader.public_key())
        .armored_message(mime)
        .unwrap();

    serde_json::from_value(json!({
        "ID": "msg-mime",
        "AddressID": ADDRESS_ID,
        "Subject": "PGP/MIME",
        "Sender": {"Address": SENDER, "Name": "Writer"},
        "Body": body,
        "MIMEType": "multipart/mixed"
    }))
    .unwrap()
}

fn signed_mime(signed_part: &str, signature: &str) -> String {
    format!(
        "Content-Type: multipart/signed; boundary=\"s\"; micalg=pgp-sha256; \
         protocol=\"application/pgp-signature\"\r\n\r\n\
         --s\r\n{signed_part}\r\n\
         --s\r\nContent-Type: application/pgp-signature; name=\"signature.asc\"\r\n\r\n\
         {signature}\r\n--s--\r\n"
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn pgp_mime_verifies_canonical_part() {
    let signed = signed_in(|_: Request| not_found()).await;
    let pgp = Pgp::new();
    let writer = new_key(&pgp, SENDER);
    let stranger = new_key(&pgp, "someone@else.test");
    let signature = pgp
        .sign()
        .with_key(writer.clone())
        .armored_detached(SIGNED_PART)
        .unwrap();

    let genuine = signed_mime(SIGNED_PART, &signature);
    let forged = signed_mime(&SIGNED_PART.replace("signed", "forged"), &signature);

    let cases: [(&str, Vec<PublicKey>, bool); 3] = [
        (&genuine, vec![writer.public_key()], true),
        (&forged, vec![writer.public_key()], false),
        (&genuine, vec![stranger.public_key()], false),
    ];

    for (mime, verifiers, valid) in cases {
        let message = encrypted_message(&pgp, &signed.address_key, mime);
        let decrypted = signed
            .client
            .decrypt_message_with(&message, &verifiers)
            .unwrap();

        assert_eq!(decrypted.mime_type(), &MimeType::MultipartMixed);
        assert!(decrypted.mime().is_some());
        assert_eq!(decrypted.is_verified(), valid);
        assert_eq!(
            decrypted
                .content()
                .signature()
                .map(|status| status.is_valid()),
            Some(valid)
        );
        assert!(decrypted.content().attachments().is_empty());
    }

    let message = encrypted_message(&pgp, &signed.address_key, &genuine);
    let decrypted = signed
        .client
        .decrypt_message_with(&message, &[writer.public_key()])
        .unwrap();
    assert_eq!(decrypted.content().text(), "signed words");
}

#[tokio::test(flavor = "multi_thread")]
async fn inner_encrypted_part_decrypts() {
    let signed = signed_in(|_: Request| not_found()).await;
    let pgp = Pgp::new();
    let writer = new_key(&pgp, SENDER);

    let inner = pgp
        .encrypt()
        .to(signed.address_key.public_key())
        .signed_by(writer.clone())
        .armored_message("Content-Type: text/html\r\n\r\n<p>inner words</p>")
        .unwrap();
    let mime = format!(
        "Content-Type: multipart/encrypted; boundary=\"e\"; protocol=\"application/pgp-encrypted\"\r\n\r\n\
         --e\r\nContent-Type: application/pgp-encrypted\r\n\r\nVersion: 1\r\n\
         --e\r\nContent-Type: application/octet-stream\r\n\r\n{inner}\r\n--e--\r\n"
    );

    let message = encrypted_message(&pgp, &signed.address_key, &mime);
    let decrypted = signed
        .client
        .decrypt_message_with(&message, &[writer.public_key()])
        .unwrap();
    let content = decrypted.content();

    assert_eq!(content.html(), Some("<p>inner words</p>"));
    assert_eq!(content.text(), "inner words");
    assert!(content.attachments().is_empty());
    assert!(decrypted.is_verified());
}
