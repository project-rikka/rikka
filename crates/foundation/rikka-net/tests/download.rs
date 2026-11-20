use rikka_sync::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use rikka_net::{CancelToken, Checksum, Client, Error, Flow, Progress, RetryPolicy};

use crate::support::{Request, response, spawn, truncated};
use tempfile::TempDir;

const BODY: &[u8] = b"rikka-os network layer payload";
const DIGEST: &str = "6885cd2bc0da82408a485769c940ca04f56a351bbeda11678e0c190e40228cfa";

#[tokio::test]
async fn download_reports_progress() {
    let url = spawn(|request: Request| {
        assert_eq!(request.header("x-app"), Some("rikka"));
        assert_eq!(request.header("authorization"), Some("Bearer token"));
        Some(response(200, &[], BODY))
    })
    .await;
    let directory = TempDir::new().expect("a temporary directory");
    let destination = directory.path().join("payload.bin");

    let reports = Arc::new(Mutex::new(Vec::<Progress>::new()));
    let seen = Arc::clone(&reports);

    let summary = Client::new()
        .download(&url, &destination)
        .header("x-app", "rikka")
        .bearer_auth("token")
        .on_progress(move |progress| {
            seen.lock().push(progress);
            Flow::Continue
        })
        .start()
        .await
        .expect("the download succeeds");

    assert_eq!(summary.bytes, BODY.len() as u64);
    assert!(!summary.resumed);
    assert!(!summary.skipped);
    assert_eq!(
        tokio::fs::read(&destination)
            .await
            .expect("the file exists"),
        BODY
    );

    let reports = reports.lock();
    let last = reports.last().expect("progress was reported");
    assert_eq!(last.transferred, BODY.len() as u64);
    assert_eq!(last.fraction(), Some(1.0));

    assert!(!directory.path().join("payload.bin.part").exists());
    assert!(!directory.path().join("payload.bin.part.meta").exists());
}

#[tokio::test]
async fn download_resumes_partial_file() {
    let attempts = Arc::new(AtomicU32::new(0));
    let seen = Arc::clone(&attempts);
    let split = 8;

    let url = spawn(move |request: Request| {
        Some(match seen.fetch_add(1, Ordering::SeqCst) {
            0 => truncated(
                200,
                &[("etag", "\"v1\""), ("accept-ranges", "bytes")],
                &BODY[..split],
                BODY.len(),
            ),
            _ => {
                assert_eq!(
                    request.header("range"),
                    Some(&format!("bytes={split}-")[..])
                );
                assert_eq!(request.header("if-range"), Some("\"v1\""));

                response(
                    206,
                    &[(
                        "content-range",
                        &format!("bytes {split}-{}/{}", BODY.len() - 1, BODY.len()),
                    )],
                    &BODY[split..],
                )
            }
        })
    })
    .await;

    let directory = TempDir::new().expect("a temporary directory");
    let destination = directory.path().join("payload.bin");

    let summary = Client::new()
        .download(&url, &destination)
        .retry(RetryPolicy::new().initial_delay(Duration::from_millis(1)))
        .start()
        .await
        .expect("the download resumes and finishes");

    assert_eq!(summary.bytes, BODY.len() as u64);
    assert_eq!(summary.status, rikka_net::StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        tokio::fs::read(&destination)
            .await
            .expect("the file exists"),
        BODY
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn download_restarts_when_range_rejected() {
    let attempts = Arc::new(AtomicU32::new(0));
    let seen = Arc::clone(&attempts);

    let url = spawn(move |request: Request| {
        Some(match seen.fetch_add(1, Ordering::SeqCst) {
            0 => truncated(200, &[("etag", "\"v1\"")], &BODY[..8], BODY.len()),
            1 => {
                assert!(request.header("range").is_some());
                response(416, &[], b"")
            }
            _ => {
                assert_eq!(request.header("range"), None);
                response(200, &[], BODY)
            }
        })
    })
    .await;

    let directory = TempDir::new().expect("a temporary directory");
    let destination = directory.path().join("payload.bin");

    let summary = Client::new()
        .download(&url, &destination)
        .retry(RetryPolicy::new().initial_delay(Duration::from_millis(1)))
        .start()
        .await
        .expect("the download starts over and finishes");

    assert_eq!(summary.bytes, BODY.len() as u64);
    assert_eq!(
        tokio::fs::read(&destination)
            .await
            .expect("the file exists"),
        BODY
    );
}

#[tokio::test]
async fn download_rejects_checksum_mismatch() {
    let url = spawn(|_| Some(response(200, &[], b"something else"))).await;
    let directory = TempDir::new().expect("a temporary directory");
    let destination = directory.path().join("payload.bin");

    let error = Client::new()
        .download(&url, &destination)
        .checksum(Checksum::sha256(DIGEST).expect("the digest parses"))
        .start()
        .await
        .expect_err("the digest does not match");

    assert!(matches!(error, Error::ChecksumMismatch { .. }), "{error:?}");
    assert!(!destination.exists());
    assert!(!directory.path().join("payload.bin.part").exists());
}

#[tokio::test]
async fn download_stops_on_cancel() {
    let url = spawn(|_| Some(response(200, &[], BODY))).await;
    let directory = TempDir::new().expect("a temporary directory");
    let destination = directory.path().join("payload.bin");

    let cancel = CancelToken::new();
    cancel.cancel();

    let error = Client::new()
        .download(&url, &destination)
        .cancel_token(cancel)
        .start()
        .await
        .expect_err("a cancelled token stops the download");

    assert!(error.is_cancelled(), "{error:?}");
    assert!(!destination.exists());
}
