use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use futures_util::StreamExt;
use rikka_net::{
    CancelToken, Client, Error, HeaderMap, HeaderValue, RetryPolicy, StatusCode, header,
};
use serde::{Deserialize, Serialize};

use crate::support::{Request, response, spawn};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Greeting {
    message: String,
}

#[tokio::test]
async fn sends_and_reads_json() {
    let url = spawn(|request: Request| {
        assert_eq!(request.method, "POST");
        assert_eq!(request.header("content-type"), Some("application/json"));
        assert_eq!(request.text(), r#"{"message":"ping"}"#);

        Some(response(
            200,
            &[("content-type", "application/json")],
            br#"{"message":"pong"}"#,
        ))
    })
    .await;

    let reply: Greeting = Client::new()
        .post_json(
            &url,
            &Greeting {
                message: "ping".to_owned(),
            },
        )
        .await
        .expect("the round trip succeeds");

    assert_eq!(reply.message, "pong");
}

#[tokio::test]
async fn sends_headers_query_and_authorization() {
    let url = spawn(|request: Request| {
        assert_eq!(request.path, "/search?q=rikka&page=2");
        assert_eq!(request.header("authorization"), Some("Bearer secret"));
        assert_eq!(request.header("x-trace"), Some("42"));
        assert!(
            request
                .header("user-agent")
                .is_some_and(|agent| agent.starts_with("rikka-net/"))
        );
        Some(response(200, &[], b""))
    })
    .await;

    let status = Client::new()
        .get(format!("{url}/search"))
        .query(&[("q", "rikka"), ("page", "2")])
        .bearer_auth("secret")
        .header("x-trace", "42")
        .send()
        .await
        .expect("the request is sent")
        .status();

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn error_status_is_ok_until_checked() {
    let url = spawn(|_| Some(response(404, &[], b"no such thing"))).await;

    let response = Client::new()
        .get(url)
        .send()
        .await
        .expect("a 404 is a response, not a failure");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let error = response
        .error_for_status()
        .await
        .expect_err("the status is turned into an error");

    assert_eq!(error.status(), Some(StatusCode::NOT_FOUND));
    let Error::Status(status) = &error else {
        panic!("expected a status error, got {error:?}");
    };
    assert_eq!(status.body(), "no such thing");
}

#[tokio::test]
async fn retries_transient_status_with_retry_after() {
    let attempts = Arc::new(AtomicU32::new(0));
    let seen = Arc::clone(&attempts);

    let url = spawn(move |_| {
        let attempt = seen.fetch_add(1, Ordering::SeqCst);
        Some(match attempt {
            0 => response(503, &[("retry-after", "0")], b"busy"),
            _ => response(200, &[], b"ready"),
        })
    })
    .await;

    let body = Client::new()
        .get(url)
        .send()
        .await
        .expect("the retry succeeds")
        .text()
        .await
        .expect("the body is read");

    assert_eq!(body, "ready");
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn retry_after_http_date_beyond_limit_stops_retrying() {
    let attempts = Arc::new(AtomicU32::new(0));
    let seen = Arc::clone(&attempts);

    let url = spawn(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
        Some(response(
            503,
            &[("retry-after", "Thu, 31 Dec 2099 23:59:59 GMT")],
            b"busy",
        ))
    })
    .await;

    let response = Client::new()
        .get(url)
        .send()
        .await
        .expect("the last response is returned");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn post_is_not_retried_by_default() {
    let attempts = Arc::new(AtomicU32::new(0));
    let seen = Arc::clone(&attempts);

    let url = spawn(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
        Some(response(503, &[("retry-after", "0")], b"busy"))
    })
    .await;

    let status = Client::new()
        .post(url)
        .text("payload")
        .send()
        .await
        .expect("the response arrives")
        .status();

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn retries_dropped_connection() {
    let attempts = Arc::new(AtomicU32::new(0));
    let seen = Arc::clone(&attempts);

    let url = spawn(move |_| {
        let attempt = seen.fetch_add(1, Ordering::SeqCst);
        (attempt > 0).then(|| response(200, &[], b"second time lucky"))
    })
    .await;

    let body = Client::new()
        .get(url)
        .retry(RetryPolicy::new().initial_delay(Duration::from_millis(1)))
        .send()
        .await
        .expect("the retry succeeds")
        .text()
        .await
        .expect("the body is read");

    assert_eq!(body, "second time lucky");
}

#[tokio::test]
async fn cancel_after_head_stops_body() {
    let url = spawn(|_| Some(response(200, &[], b"hello"))).await;
    let cancel = CancelToken::new();

    let answered = Client::new()
        .get(&url)
        .cancel_token(cancel.clone())
        .send()
        .await
        .expect("the head arrives");
    cancel.cancel();

    let mut body = Box::pin(answered.bytes_stream());
    let error = body
        .next()
        .await
        .expect("the stream says why it ended")
        .expect_err("it ended because it was cancelled");
    assert!(error.is_cancelled(), "{error:?}");
}

#[tokio::test]
async fn body_replaces_content_type() {
    let url = spawn(|request: Request| {
        let named = request
            .headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .count();

        assert_eq!(named, 1, "the content type was sent {named} times");
        assert_eq!(request.header("content-type"), Some("application/json"));
        assert_eq!(request.header("authorization"), Some("Bearer notarealkey"));

        Some(response(200, &[], b"{}"))
    })
    .await;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer notarealkey"),
    );

    Client::new()
        .post(url)
        .headers(headers)
        .json(&Greeting {
            message: "ping".to_owned(),
        })
        .send()
        .await
        .expect("the request is sent");
}

#[tokio::test]
async fn bounded_read_rejects_oversized_body() {
    let declared = spawn(|_| Some(response(200, &[], b"0123456789"))).await;
    let undeclared =
        spawn(|_| Some(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n0123456789".to_vec())).await;
    let client = Client::new();

    for url in [&declared, &undeclared] {
        let refused = client.get(url).send().await.unwrap().bytes_limited(4).await;
        assert!(
            matches!(refused, Err(Error::TooLarge { limit: 4 })),
            "{url}"
        );

        let read = client
            .get(url)
            .send()
            .await
            .unwrap()
            .bytes_limited(10)
            .await
            .unwrap();
        assert_eq!(&read[..], b"0123456789");
    }
}
