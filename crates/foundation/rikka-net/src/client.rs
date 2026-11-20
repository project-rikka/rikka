use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http::{HeaderMap, Method};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::cancel::{self, CancelToken};
use crate::client_builder::ClientBuilder;
use crate::download::DownloadBuilder;
use crate::error::{Error, Result, exhausted, is_transient_status};
use crate::into_url::IntoUrl;
use crate::request::RequestBuilder;
use crate::response::Response;
use crate::retry::{self, RetryPolicy};

use crate::tls::SocketTls;
use crate::upload::UploadBuilder;
use crate::websocket::WebSocketBuilder;

#[derive(Debug, Clone)]
pub struct Client {
    shared: Arc<Shared>,
}

#[derive(Debug)]
pub(crate) struct Shared {
    pub(crate) http: reqwest::Client,
    pub(crate) retry: RetryPolicy,
    pub(crate) headers: HeaderMap,
    pub(crate) socket_tls: SocketTls,
    pub(crate) connect_timeout: Option<Duration>,
}

impl Client {
    #[must_use]
    pub fn new() -> Client {
        ClientBuilder::new()
            .build()
            .expect("failed to initialize default TLS backend")
    }

    #[must_use]
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    pub(crate) fn from_shared(shared: Shared) -> Client {
        Client {
            shared: Arc::new(shared),
        }
    }

    pub(crate) fn shared(&self) -> &Shared {
        &self.shared
    }

    #[must_use]
    pub fn request(&self, method: Method, url: impl IntoUrl) -> RequestBuilder {
        let inner = url
            .into_url()
            .map(|url| self.shared.http.request(method, url));
        RequestBuilder::new(self.clone(), inner)
    }

    #[must_use]
    pub fn get(&self, url: impl IntoUrl) -> RequestBuilder {
        self.request(Method::GET, url)
    }

    #[must_use]
    pub fn post(&self, url: impl IntoUrl) -> RequestBuilder {
        self.request(Method::POST, url)
    }

    #[must_use]
    pub fn put(&self, url: impl IntoUrl) -> RequestBuilder {
        self.request(Method::PUT, url)
    }

    #[must_use]
    pub fn patch(&self, url: impl IntoUrl) -> RequestBuilder {
        self.request(Method::PATCH, url)
    }

    #[must_use]
    pub fn delete(&self, url: impl IntoUrl) -> RequestBuilder {
        self.request(Method::DELETE, url)
    }

    #[must_use]
    pub fn head(&self, url: impl IntoUrl) -> RequestBuilder {
        self.request(Method::HEAD, url)
    }

    #[must_use]
    pub fn download(&self, url: impl IntoUrl, destination: impl AsRef<Path>) -> DownloadBuilder {
        DownloadBuilder::new(
            self.clone(),
            url.into_url(),
            destination.as_ref().to_owned(),
        )
    }

    #[must_use]
    pub fn upload(&self, url: impl IntoUrl, source: impl AsRef<Path>) -> UploadBuilder {
        UploadBuilder::new(self.clone(), url.into_url(), source.as_ref().to_owned())
    }

    #[must_use]
    pub fn websocket(&self, url: impl IntoUrl) -> WebSocketBuilder {
        WebSocketBuilder::new(self.clone(), url.into_url())
    }

    #[must_use]
    pub fn blocking(&self) -> crate::blocking::Client {
        crate::blocking::Client::from_async(self.clone())
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: impl IntoUrl) -> Result<T> {
        self.get(url).json_response().await
    }

    pub async fn post_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        url: impl IntoUrl,
        body: &B,
    ) -> Result<T> {
        self.post(url).json(body).json_response().await
    }

    pub async fn get_text(&self, url: impl IntoUrl) -> Result<String> {
        self.get(url)
            .send()
            .await?
            .error_for_status()
            .await?
            .text()
            .await
    }

    pub async fn get_bytes(&self, url: impl IntoUrl) -> Result<Bytes> {
        self.get(url)
            .send()
            .await?
            .error_for_status()
            .await?
            .bytes()
            .await
    }

    pub(crate) async fn execute(
        &self,
        mut request: reqwest::Request,
        retry: Option<RetryPolicy>,
        cancel: Option<CancelToken>,
    ) -> Result<Response> {
        let policy = retry.unwrap_or_else(|| self.shared.retry.clone());
        let repeatable = policy.allows(request.method());
        let cancel = cancel.as_ref();
        let mut attempt = 0u32;

        loop {
            let again = repeatable.then(|| request.try_clone()).flatten();

            let sent = cancel::until_cancelled(self.shared.http.execute(request), cancel).await?;

            let (next, delay) = match sent {
                Ok(response) => {
                    if !is_transient_status(response.status()) {
                        return Ok(Response::new(response, cancel.cloned()));
                    }

                    let hint = retry::retry_after(response.headers());
                    let Some((next, delay)) = again.zip(policy.delay(attempt, hint)) else {
                        return Ok(Response::new(response, cancel.cloned()));
                    };

                    tracing::debug!(
                        status = %response.status(),
                        attempt = attempt + 1,
                        ?delay,
                        "retrying after transient status",
                    );
                    (next, delay)
                }
                Err(error) => {
                    let error = match cancel {
                        Some(token) if token.is_cancelled() => Error::Cancelled,
                        _ => Error::from(error),
                    };
                    let Some((next, delay)) = again
                        .filter(|_| error.is_transient())
                        .zip(policy.delay(attempt, None))
                    else {
                        return Err(exhausted(attempt, error));
                    };

                    tracing::debug!(%error, attempt = attempt + 1, ?delay, "retrying request");
                    (next, delay)
                }
            };

            cancel::sleep(delay, cancel).await?;
            request = next;
            attempt += 1;
        }
    }
}

impl Default for Client {
    fn default() -> Client {
        Client::new()
    }
}
