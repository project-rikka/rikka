use std::fmt;
use std::time::Duration;

use rikka_net::{Client, RetryPolicy, StatusCode};

use crate::error::{Error, Result};

const ANSWER_LIMIT: usize = 4 << 20;
const PAGE_LIMIT: usize = 2 << 20;
const TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const RETRY_PAUSE: Duration = Duration::from_millis(400);

#[derive(Clone)]
pub struct Http {
    client: Client,
}

#[must_use]
pub struct Post<'a> {
    http: &'a Http,
    url: String,
    query: Vec<(&'static str, String)>,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Http {
    pub fn new() -> Result<Http> {
        let client = Client::builder()
            .cookies(false)
            .referer(false)
            .retry(RetryPolicy::never())
            .timeout(TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(Error::Client)?;
        Ok(Http { client })
    }

    pub fn post(&self, url: impl Into<String>) -> Post<'_> {
        Post {
            http: self,
            url: url.into(),
            query: Vec::new(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub async fn page(&self, url: &str) -> Result<String> {
        let response = self.client.get(url).send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::from_status(status));
        }
        let body = response.bytes_limited(PAGE_LIMIT).await?;
        Ok(String::from_utf8_lossy(&body).into_owned())
    }
}

impl Post<'_> {
    pub fn query(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.query.push((name, value.into()));
        self
    }

    pub fn header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }

    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    pub async fn send(self) -> Result<Vec<u8>> {
        let mut retried = false;
        loop {
            let response = self.request().send().await?;
            let status = response.status();
            if status.is_success() {
                return Ok(response.bytes_limited(ANSWER_LIMIT).await?.to_vec());
            }
            if status.is_server_error() && !retried {
                retried = true;
                tokio::time::sleep(RETRY_PAUSE).await;
                continue;
            }
            return Err(Error::from_status(status));
        }
    }

    fn request(&self) -> rikka_net::RequestBuilder {
        let mut request = self
            .http
            .client
            .post(self.url.as_str())
            .body(self.body.clone());
        if !self.query.is_empty() {
            request = request.query(&self.query);
        }
        for (name, value) in &self.headers {
            request = request.header(*name, value.as_str());
        }
        request
    }
}

impl Error {
    fn from_status(status: StatusCode) -> Error {
        match status {
            StatusCode::TOO_MANY_REQUESTS => Error::RateLimited,
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Error::Refused,
            _ => Error::Status(status.as_u16()),
        }
    }
}
impl fmt::Debug for Http {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Http").finish_non_exhaustive()
    }
}

impl fmt::Debug for Post<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Post")
            .field("url", &self.url)
            .field("query", &self.query)
            .field(
                "headers",
                &self
                    .headers
                    .iter()
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>(),
            )
            .field("body", &self.body.len())
            .finish()
    }
}
