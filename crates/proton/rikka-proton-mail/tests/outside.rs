use jiff::Timestamp;
use std::sync::{Arc, Mutex};

use base64::prelude::{BASE64_STANDARD, Engine as _};
use rikka_proton_mail::{Error, OutsideClient};
use rikka_proton_pgp::{Pgp, PrivateKey};
use serde_json::json;

use rikka_test_proton::{
    Request, ResponseFuture, api, binary_response, json_response, new_key, not_found, response,
};

const LINK: &str = "eo-link-id";
const PASSWORD: &str = "shared secret";
const TOKEN: &str = "decrypted-token-value";
const FILE: &[u8] = b"quarterly numbers";

struct Outside {
    pgp: Pgp,
    sender: PrivateKey,
    data_packet: Vec<u8>,
    message: serde_json::Value,
    replies: Arc<Mutex<Vec<Request>>>,
}

impl Outside {
    fn new() -> Outside {
        let pgp = Pgp::new();
        let sender = new_key(&pgp, "sender@proton.test");

        let body = pgp
            .encrypt()
            .with_password(PASSWORD)
            .signed_by(sender.clone())
            .as_utf8()
            .armored_message("<p>for your eyes</p>")
            .unwrap();
        let (key_packets, data_packet, _) = pgp
            .encrypt()
            .with_password(PASSWORD)
            .split(FILE)
            .unwrap()
            .into_parts();
        let signature = pgp.sign().with_key(sender.clone()).detached(FILE).unwrap();
        let earlier = pgp
            .encrypt()
            .with_password(PASSWORD)
            .armored_message("an earlier reply")
            .unwrap();

        let message = json!({
            "Code": 1000,
            "Message": {
                "Subject": "Private",
                "Sender": {"Address": "sender@proton.test", "Name": "Sender"},
                "ToList": [{"Address": "friend@elsewhere.test", "Name": ""}],
                "Time": 1_700_000_000,
                "NumAttachments": 1,
                "Body": body,
                "MIMEType": "text/html",
                "Recipient": "friend@elsewhere.test",
                "Attachments": [{
                    "ID": "att-1",
                    "Name": "numbers.txt",
                    "MIMEType": "text/plain",
                    "KeyPackets": BASE64_STANDARD.encode(key_packets),
                    "Signature": BASE64_STANDARD.encode(signature)
                }],
                "Replies": [{"Body": earlier, "Time": 5, "Attachments": []}]
            },
            "PublicKey": sender.public_key().export_armored().unwrap()
        });

        Outside {
            pgp,
            sender,
            data_packet,
            message,
            replies: Arc::default(),
        }
    }

    fn handler(&self) -> impl Fn(Request) -> ResponseFuture + Send + Sync + 'static {
        let token = self
            .pgp
            .encrypt()
            .with_password(PASSWORD)
            .armored_message(TOKEN)
            .unwrap();
        let message = self.message.clone();
        let data_packet = self.data_packet.clone();
        let replies = Arc::clone(&self.replies);

        move |request: Request| {
            let authorized = request.header("authorization") == Some(TOKEN)
                && request.header("x-eo-uid") == Some(LINK);

            match request.route() {
                ("GET", "/mail/v4/eo/token/eo-link-id") => {
                    json_response(200, &json!({"Code": 1000, "Token": token}))
                }
                (_, path) if path.starts_with("/mail/v4/eo/") && !authorized => {
                    response(401, r#"{"Code":401,"Error":"Invalid token"}"#)
                }
                ("GET", "/mail/v4/eo/message") => json_response(200, &message),
                ("GET", "/mail/v4/eo/attachment/att-1") => {
                    binary_response(200, data_packet.clone())
                }
                ("POST", "/mail/v4/eo/reply") => {
                    let own_copy = request
                        .form()
                        .into_iter()
                        .find(|(name, _)| name == "ReplyBody")
                        .map(|(_, value)| String::from_utf8_lossy(&value).into_owned())
                        .unwrap_or_default();
                    replies.lock().unwrap().push(request);
                    json_response(
                        200,
                        &json!({"Code": 1000, "Reply": {"Body": own_copy, "Time": 9}}),
                    )
                }
                _ => not_found(),
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn outside_recipient_reads_and_replies() {
    let outside = Outside::new();
    let client = OutsideClient::new(api(outside.handler()).await);

    let session = client.unlock(LINK, PASSWORD).await.unwrap();
    let message = session.message().await.unwrap();

    assert_eq!(message.subject(), "Private");
    assert_eq!(message.recipient(), "friend@elsewhere.test");
    assert_eq!(message.text(), "<p>for your eyes</p>");
    assert_eq!(message.content().text(), "for your eyes");
    assert!(message.is_verified());
    assert_eq!(message.replies()[0].body(), "an earlier reply");

    let file = session
        .attachment(&message, &message.attachments()[0])
        .await
        .unwrap();
    assert_eq!(file.bytes(), FILE);
    assert!(file.is_verified());

    let response = session
        .reply(&message)
        .body("<p>thanks</p>")
        .attach("note.txt", "text/plain", b"see attached".to_vec())
        .send()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(response.body(), "<p>thanks</p>");
    assert_eq!(response.time(), Timestamp::from_second(9).unwrap());

    let sent = outside.replies.lock().unwrap()[0].form();
    let field = |header: &str| -> Vec<Vec<u8>> {
        sent.iter()
            .filter(|(name, _)| name == header)
            .map(|(_, value)| value.clone())
            .collect()
    };

    let body = outside
        .pgp
        .decrypt()
        .with_key(outside.sender.clone())
        .message(&field("Body")[0])
        .unwrap();
    assert_eq!(body.data(), b"<p>thanks</p>");
    assert_eq!(field("Filename[]"), vec![b"note.txt".to_vec()]);
    assert_eq!(field("MIMEType[]"), vec![b"text/plain".to_vec()]);

    let mut packet = field("KeyPackets[]").remove(0);
    packet.extend(field("DataPacket[]").remove(0));
    let attached = outside
        .pgp
        .decrypt()
        .with_key(outside.sender.clone())
        .message(&packet)
        .unwrap();
    assert_eq!(attached.data(), b"see attached");
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_wrong_password_or_link() {
    let outside = Outside::new();
    let client = OutsideClient::new(api(outside.handler()).await);

    let wrong = client.unlock(LINK, "not the password").await;
    assert!(matches!(wrong, Err(Error::WrongPassword)), "{wrong:?}");

    let malformed = client.unlock("../escape", PASSWORD).await;
    assert!(
        matches!(malformed, Err(Error::InvalidOutsideLink(_))),
        "{malformed:?}"
    );
}
