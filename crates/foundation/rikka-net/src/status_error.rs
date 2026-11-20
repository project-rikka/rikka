use std::time::Duration;

use http::{HeaderMap, StatusCode};
use url::Url;

use crate::retry;

pub(crate) const BODY_LIMIT: usize = 8 * 1024;

#[derive(Debug, thiserror::Error)]
#[error("HTTP {status} from {url}")]
pub struct StatusError {
    status: StatusCode,
    url: Url,
    body: String,
    retry_after: Option<Duration>,
}

impl StatusError {
    pub(crate) fn new(
        status: StatusCode,
        url: Url,
        headers: &HeaderMap,
        body: String,
    ) -> StatusError {
        StatusError {
            status,
            url,
            body,
            retry_after: retry::retry_after(headers),
        }
    }

    #[must_use]
    pub fn status(&self) -> StatusCode {
        self.status
    }

    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    #[must_use]
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Option<T> {
        serde_json::from_str(&self.body).ok()
    }

    #[must_use]
    pub fn retry_after(&self) -> Option<Duration> {
        self.retry_after
    }
}
