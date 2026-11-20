use rikka_net::StatusCode;
use rikka_net::blocking::Client;

use crate::support::{response, spawn};

fn on_a_worker_thread<T, F>(work: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    std::thread::spawn(work)
        .join()
        .expect("the thread finishes")
}

#[tokio::test(flavor = "multi_thread")]
async fn blocking_sends_request() {
    let url = spawn(|_| Some(response(200, &[], b"hello"))).await;

    let body = on_a_worker_thread(move || {
        Client::new()
            .get(&url)
            .send()
            .expect("the request is sent")
            .text()
            .expect("the body is read")
    });

    assert_eq!(body, "hello");
}

#[tokio::test(flavor = "multi_thread")]
async fn error_for_status_returns_error() {
    let url = spawn(|_| Some(response(500, &[], b"broken"))).await;

    let error = on_a_worker_thread(move || {
        Client::new()
            .get(&url)
            .send()
            .expect("the response arrives")
            .error_for_status()
            .expect_err("the status is an error")
    });

    assert_eq!(error.status(), Some(StatusCode::INTERNAL_SERVER_ERROR));
}
