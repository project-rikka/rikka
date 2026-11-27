use google_drive::Error;
use google_test_server::Response;
use serde_json::{Value, json};

use crate::support::{FOLDER, Google, file};

#[tokio::test]
async fn edits_send_the_documented_requests() {
    let google = Google::start(|request| match request.method.as_str() {
        "DELETE" => Response::status(204),
        _ => Response::json(&file("a", "answer", "text/plain").to_string()),
    });
    let drive = google.drive().await;

    drive
        .create_folder("parent", "New folder")
        .await
        .expect("a folder");
    drive.rename("a", "renamed.txt").await.expect("a rename");
    drive
        .relocate("a", &["old".to_owned()], "new")
        .await
        .expect("a move");
    drive.set_trashed("a", true).await.expect("a trashed file");
    drive.set_starred("a", true).await.expect("a starred file");
    drive.delete("a").await.expect("a deletion");
    drive.empty_trash().await.expect("an empty trash");

    let asked = google.account.calls();
    let body = |index: usize| serde_json::from_slice::<Value>(&asked[index].body).expect("json");
    let shape = |index: usize| (asked[index].method.as_str(), asked[index].path());
    assert_eq!(shape(0), ("POST", "/drive/v3/files"));
    assert_eq!(
        body(0),
        json!({ "name": "New folder", "mimeType": FOLDER, "parents": ["parent"] })
    );
    assert_eq!(shape(1), ("PATCH", "/drive/v3/files/a"));
    assert_eq!(body(1), json!({ "name": "renamed.txt" }));
    assert_eq!(asked[2].query()["addParents"], "new");
    assert_eq!(asked[2].query()["removeParents"], "old");
    assert_eq!(body(3), json!({ "trashed": true }));
    assert_eq!(body(4), json!({ "starred": true }));
    assert_eq!(shape(5), ("DELETE", "/drive/v3/files/a"));
    assert_eq!(shape(6), ("DELETE", "/drive/v3/files/trash"));
    assert!(
        asked
            .iter()
            .take(6)
            .all(|request| request.query()["supportsAllDrives"] == "true")
    );
}

#[tokio::test]
async fn refusals_become_semantic_errors() {
    let google = Google::start(|request| {
        let failure = |status: u16, reason: &str| {
            Response::status(status).body(
                json!({ "error": {
                    "code": status,
                    "message": "refused",
                    "errors": [{ "reason": reason }],
                } })
                .to_string(),
            )
        };
        match request.path() {
            "/drive/v3/files/gone" => failure(404, "notFound"),
            "/drive/v3/files/full" => failure(403, "storageQuotaExceeded"),
            "/drive/v3/files/busy" => failure(403, "userRateLimitExceeded"),
            "/drive/v3/files/locked" => failure(403, "insufficientFilePermissions"),
            _ => Response::status(500).body("boom"),
        }
    });
    let drive = google.drive().await;

    let rename = |id: &'static str| drive.rename(id, "name");
    assert!(matches!(rename("gone").await, Err(Error::Missing)));
    assert!(matches!(rename("full").await, Err(Error::StorageFull)));
    assert!(matches!(rename("busy").await, Err(Error::RateLimited)));
    assert!(
        matches!(rename("locked").await, Err(Error::Forbidden(message)) if message == "refused")
    );
    assert!(matches!(rename("other").await, Err(Error::Api { message, .. }) if message == "boom"));
}
