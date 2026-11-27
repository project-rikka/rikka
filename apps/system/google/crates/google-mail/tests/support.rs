use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use google_mail::Mail;
use google_test_server::{Account, Request, Response};
use serde_json::{Value, json};

pub(crate) struct Google {
    pub(crate) account: Account,
}

impl Google {
    pub(crate) fn start(gmail: impl Fn(&Request) -> Response + Send + Sync + 'static) -> Google {
        Google {
            account: Account::start(gmail),
        }
    }

    pub(crate) async fn mail(&self) -> Mail {
        Mail::new(
            rikka_net::Client::new(),
            self.account.grants().await,
            &self.account.server.url("/gmail/v1/users/me"),
            &self.account.server.url("/upload/gmail/v1/users/me"),
        )
    }
}

pub(crate) fn data(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

pub(crate) fn header(name: &str, value: &str) -> Value {
    json!({ "name": name, "value": value })
}

pub(crate) fn metadata(id: &str, from: &str, subject: &str, labels: &[&str]) -> Value {
    json!({
        "id": id,
        "threadId": "thread",
        "labelIds": labels,
        "snippet": "It&#39;s here",
        "internalDate": "1700000000000",
        "payload": {
            "mimeType": "multipart/mixed",
            "headers": [header("From", from), header("Subject", subject)],
        },
    })
}
