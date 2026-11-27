use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use google_drive::{Error, Export, File, Kind};
use google_test_server::Response;
use rikka_net::CancelToken;

use crate::support::{Google, file};

fn parsed(value: serde_json::Value) -> File {
    serde_json::from_value(value).expect("a file")
}

#[tokio::test]
async fn download_writes_the_media_and_reports_progress() {
    let google = Google::start(
        |request| match request.query().get("alt").map(String::as_str) {
            Some("media") => Response::ok(vec![7_u8; 4096]),
            _ => Response::status(400),
        },
    );
    let drive = google.drive().await;
    let folder = tempfile::tempdir().unwrap();
    let destination = folder.path().join("photo.jpg");
    let moved = Arc::new(AtomicU64::new(0));
    let seen = Arc::clone(&moved);

    drive
        .download(
            &parsed(file("a", "photo.jpg", "image/jpeg")),
            &destination,
            CancelToken::new(),
            move |progress| seen.store(progress.transferred, Ordering::Release),
        )
        .await
        .expect("a download");

    assert_eq!(
        std::fs::read(&destination).expect("the file"),
        vec![7_u8; 4096]
    );
    assert_eq!(moved.load(Ordering::Acquire), 4096);
    assert_eq!(google.asked("/drive/v3/files/a").len(), 1);
}

#[tokio::test]
async fn native_documents_export_instead_of_downloading() {
    let google = Google::start(|request| match request.path() {
        "/drive/v3/files/doc/export" => Response::ok("%PDF"),
        _ => Response::status(400),
    });
    let drive = google.drive().await;
    let folder = tempfile::tempdir().unwrap();
    let destination = folder.path().join("doc.pdf");
    let document = parsed(file("doc", "Plan", "application/vnd.google-apps.document"));

    let refused = drive
        .download(&document, &destination, CancelToken::new(), |_| {})
        .await;
    drive
        .export(&document, Export::PDF, &destination, CancelToken::new())
        .await
        .expect("an export");

    assert!(matches!(refused, Err(Error::NotDownloadable)));
    assert_eq!(std::fs::read(&destination).expect("the file"), b"%PDF");
    assert_eq!(
        google.asked("/drive/v3/files/doc/export")[0].query()["mimeType"],
        "application/pdf"
    );
    assert_eq!(
        Export::editable(Kind::Document).map(|export| export.extension),
        Some("docx")
    );
    assert_eq!(Export::editable(Kind::Image), None);
}

#[tokio::test]
async fn upload_opens_a_session_then_sends_the_bytes() {
    let google = Google::start(|request| match (request.method.as_str(), request.path()) {
        ("POST", "/upload/drive/v3/files") => {
            let host = request.header("host").unwrap_or_default();
            Response::ok("").header("Location", &format!("http://{host}/upload/session-1"))
        }
        ("PUT", "/upload/session-1") => {
            Response::json(&file("new", "report.txt", "text/plain").to_string())
        }
        _ => Response::status(400),
    });
    let drive = google.drive().await;
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("report.txt");
    std::fs::write(&source, b"quarterly numbers").unwrap();

    let stored = drive
        .upload(&source, "parent", "report.txt", CancelToken::new(), |_| {})
        .await
        .expect("an upload");

    assert_eq!(stored.id, "new");
    let opened = &google.asked("/upload/drive/v3/files")[0];
    assert_eq!(opened.query()["uploadType"], "resumable");
    assert_eq!(opened.header("x-upload-content-length"), Some("17"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&opened.body).expect("metadata"),
        serde_json::json!({ "name": "report.txt", "parents": ["parent"] })
    );
    assert_eq!(
        google.asked("/upload/session-1")[0].body,
        b"quarterly numbers"
    );
}

#[tokio::test]
async fn missing_media_is_reported_as_missing() {
    let google = Google::start(|_| Response::status(404));
    let drive = google.drive().await;
    let folder = tempfile::tempdir().unwrap();

    let missing = drive
        .download(
            &parsed(file("gone", "gone.txt", "text/plain")),
            &folder.path().join("gone.txt"),
            CancelToken::new(),
            |_| {},
        )
        .await;

    assert!(matches!(missing, Err(Error::Missing)));
}
