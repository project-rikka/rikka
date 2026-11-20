use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::time::Duration;

use serde_json::{Value, json};

pub type ResponseFuture = Pin<Box<dyn Future<Output = Response> + Send>>;

#[derive(Debug, Clone)]
pub struct Response {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

impl Response {
    #[must_use]
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Response {
        Response {
            status,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    #[must_use]
    pub fn json(status: u16, body: &Value) -> Response {
        Response::new(status, body.to_string())
    }

    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Response {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
}

impl IntoFuture for Response {
    type Output = Response;
    type IntoFuture = ResponseFuture;

    fn into_future(self) -> ResponseFuture {
        Box::pin(std::future::ready(self))
    }
}

pub fn response(status: u16, body: impl Into<Vec<u8>>) -> ResponseFuture {
    Response::new(status, body).into_future()
}

pub fn json_response(status: u16, body: &Value) -> ResponseFuture {
    Response::json(status, body).into_future()
}

pub fn binary_response(status: u16, body: Vec<u8>) -> ResponseFuture {
    Response::new(status, body)
        .header("Content-Type", "application/octet-stream")
        .into_future()
}

pub fn success() -> ResponseFuture {
    json_response(200, &json!({"Code": 1000}))
}

pub fn not_found() -> ResponseFuture {
    json_response(404, &json!({"Code": 2501, "Error": "Not found"}))
}

pub fn delayed(delay: Duration, response: ResponseFuture) -> ResponseFuture {
    Box::pin(async move {
        tokio::time::sleep(delay).await;
        response.await
    })
}
