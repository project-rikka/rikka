use browser_translate::{Error, Http};

use crate::support::{Server, reply};

#[tokio::test]
async fn post_sends_query_headers_body() {
    let server = Server::start(|_| reply("200 OK", "answer"));
    let http = Http::new().expect("create http client");
    let body = http
        .post(server.url("/translate"))
        .query("to", "zh-Hans")
        .query("textType", "html")
        .header("content-type", "application/json")
        .body(b"[\"Hello\"]".to_vec())
        .send()
        .await
        .expect("translation response");
    assert_eq!(body, b"answer");
    let recorded = server.recorded();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].method, "POST");
    assert_eq!(recorded[0].target, "/translate?to=zh-Hans&textType=html");
    assert_eq!(recorded[0].header("content-type"), Some("application/json"));
    assert_eq!(recorded[0].body, "[\"Hello\"]");
    assert_eq!(recorded[0].header("cookie"), None);
    assert_eq!(recorded[0].header("referer"), None);
}

#[tokio::test]
async fn maps_statuses_to_errors() {
    type Expected = fn(&Error) -> bool;
    let cases: [(&str, Expected); 4] = [
        ("429 Too Many Requests", |error| {
            matches!(error, Error::RateLimited)
        }),
        ("401 Unauthorized", |error| matches!(error, Error::Refused)),
        ("403 Forbidden", |error| matches!(error, Error::Refused)),
        ("404 Not Found", |error| matches!(error, Error::Status(404))),
    ];
    let http = Http::new().expect("create http client");
    for (status, expected) in cases {
        let server = Server::start(move |_| reply(status, ""));
        let error = http
            .post(server.url("/"))
            .send()
            .await
            .expect_err("request fails");
        assert!(expected(&error), "{status}: {error:?}");
        assert_eq!(server.recorded().len(), 1, "{status} was retried");
    }
}

#[tokio::test]
async fn rejects_oversized_responses() {
    let http = Http::new().expect("create http client");
    let declaring = |_: &crate::support::Recorded| {
        "HTTP/1.1 200 OK\r\nContent-Length: 1073741824\r\nConnection: close\r\n\r\n".to_owned()
    };
    let answers = Server::start(declaring);
    let error = http
        .post(answers.url("/"))
        .send()
        .await
        .expect_err("too large");
    assert!(matches!(error, Error::ResponseTooLarge(_)), "{error:?}");
    let pages = Server::start(declaring);
    let error = http.page(&pages.url("/")).await.expect_err("too large");
    assert!(matches!(error, Error::ResponseTooLarge(_)), "{error:?}");
}
