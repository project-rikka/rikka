use std::net::SocketAddr;

use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use http::{HeaderMap, StatusCode, Version, header};
use serde::de::DeserializeOwned;
use url::Url;

use crate::cancel::{self, CancelToken};
use crate::error::{Error, Result};
use crate::status_error::{BODY_LIMIT, StatusError};

#[derive(Debug)]
pub struct Response {
    inner: reqwest::Response,
    cancel: Option<CancelToken>,
}

impl Response {
    pub(crate) fn new(inner: reqwest::Response, cancel: Option<CancelToken>) -> Response {
        Response { inner, cancel }
    }

    #[must_use]
    pub fn status(&self) -> StatusCode {
        self.inner.status()
    }

    #[must_use]
    pub fn is_success(&self) -> bool {
        self.status().is_success()
    }

    #[must_use]
    pub fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    #[must_use]
    pub fn url(&self) -> &Url {
        self.inner.url()
    }

    #[must_use]
    pub fn version(&self) -> Version {
        self.inner.version()
    }

    #[must_use]
    pub fn content_length(&self) -> Option<u64> {
        self.inner.content_length()
    }

    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
    }

    #[must_use]
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.inner.remote_addr()
    }

    pub async fn error_for_status(self) -> Result<Response> {
        let status = self.status();
        if !status.is_client_error() && !status.is_server_error() {
            return Ok(self);
        }

        let url = self.url().clone();
        let headers = self.headers().clone();
        let body = read_error_body(self.inner).await;
        Err(StatusError::new(status, url, &headers, body).into())
    }

    pub async fn bytes(self) -> Result<Bytes> {
        let Response { inner, cancel } = self;
        Ok(cancel::until_cancelled(inner.bytes(), cancel.as_ref()).await??)
    }

    pub async fn bytes_limited(mut self, limit: usize) -> Result<Bytes> {
        if self
            .content_length()
            .is_some_and(|length| length > limit as u64)
        {
            return Err(Error::TooLarge { limit });
        }
        let mut body = Vec::new();
        while let Some(chunk) = self.chunk().await? {
            if body.len() + chunk.len() > limit {
                return Err(Error::TooLarge { limit });
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body.into())
    }

    pub async fn text(self) -> Result<String> {
        let Response { inner, cancel } = self;
        Ok(cancel::until_cancelled(inner.text(), cancel.as_ref()).await??)
    }

    pub async fn json<T: DeserializeOwned>(self) -> Result<T> {
        let bytes = self.bytes().await?;
        serde_json::from_slice(&bytes).map_err(|error| Error::Decode {
            format: "JSON",
            source: Box::new(error),
        })
    }

    pub async fn chunk(&mut self) -> Result<Option<Bytes>> {
        Ok(cancel::until_cancelled(self.inner.chunk(), self.cancel.as_ref()).await??)
    }

    pub fn bytes_stream(self) -> impl Stream<Item = Result<Bytes>> + Send + 'static {
        let Response { inner, cancel } = self;
        let reading = (Box::pin(inner.bytes_stream()), cancel);
        futures_util::stream::unfold(Some(reading), |reading| async move {
            let (mut chunks, cancel) = reading?;
            match cancel::until_cancelled(chunks.next(), cancel.as_ref()).await {
                Err(cancelled) => Some((Err(cancelled), None)),
                Ok(None) => None,
                Ok(Some(chunk)) => Some((chunk.map_err(Error::from), Some((chunks, cancel)))),
            }
        })
    }
}

async fn read_error_body(mut response: reqwest::Response) -> String {
    let mut body = Vec::new();

    while body.len() < BODY_LIMIT {
        match response.chunk().await {
            Ok(Some(chunk)) => body.extend_from_slice(&chunk),
            Ok(None) | Err(_) => break,
        }
    }

    body.truncate(BODY_LIMIT);
    String::from_utf8_lossy(&body).into_owned()
}
