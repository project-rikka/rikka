use google_drive::Drive;
use google_test_server::{Account, Request, Response};

pub(crate) const FOLDER: &str = "application/vnd.google-apps.folder";

pub(crate) struct Google {
    pub(crate) account: Account,
}

impl Google {
    pub(crate) fn start(drive: impl Fn(&Request) -> Response + Send + Sync + 'static) -> Google {
        Google {
            account: Account::start(drive),
        }
    }

    pub(crate) async fn drive(&self) -> Drive {
        Drive::new(
            rikka_net::Client::new(),
            self.account.grants().await,
            &self.account.server.url("/drive/v3"),
            &self.account.server.url("/upload/drive/v3"),
        )
    }

    pub(crate) fn asked(&self, path: &str) -> Vec<Request> {
        self.account.asked(path)
    }
}

pub(crate) fn file(id: &str, name: &str, mime: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "name": name,
        "mimeType": mime,
        "size": "1024",
        "version": "7",
        "modifiedTime": "2023-11-14T22:13:20.000Z",
        "parents": ["root-id"],
        "capabilities": { "canEdit": true, "canRename": true, "canDownload": true },
    })
}
