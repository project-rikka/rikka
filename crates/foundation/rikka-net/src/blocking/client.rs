use std::path::Path;

use bytes::Bytes;
use http::Method;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::blocking::request::BlockingRequestBuilder;
use crate::blocking::websocket::BlockingWebSocketBuilder;
use crate::download::DownloadBuilder;
use crate::error::Result;
use crate::into_url::IntoUrl;
use crate::runtime;
use crate::upload::UploadBuilder;

#[derive(Debug, Clone)]
pub struct Client {
    inner: crate::client::Client,
}

impl Client {
    #[must_use]
    pub fn new() -> Client {
        Client::from_async(crate::client::Client::new())
    }

    pub(crate) fn from_async(inner: crate::client::Client) -> Client {
        Client { inner }
    }

    #[must_use]
    pub fn into_async(self) -> crate::client::Client {
        self.inner
    }

    #[must_use]
    pub fn request(&self, method: Method, url: impl IntoUrl) -> BlockingRequestBuilder {
        BlockingRequestBuilder::new(self.inner.request(method, url))
    }

    #[must_use]
    pub fn get(&self, url: impl IntoUrl) -> BlockingRequestBuilder {
        self.request(Method::GET, url)
    }

    #[must_use]
    pub fn post(&self, url: impl IntoUrl) -> BlockingRequestBuilder {
        self.request(Method::POST, url)
    }

    #[must_use]
    pub fn put(&self, url: impl IntoUrl) -> BlockingRequestBuilder {
        self.request(Method::PUT, url)
    }

    #[must_use]
    pub fn patch(&self, url: impl IntoUrl) -> BlockingRequestBuilder {
        self.request(Method::PATCH, url)
    }

    #[must_use]
    pub fn delete(&self, url: impl IntoUrl) -> BlockingRequestBuilder {
        self.request(Method::DELETE, url)
    }

    #[must_use]
    pub fn head(&self, url: impl IntoUrl) -> BlockingRequestBuilder {
        self.request(Method::HEAD, url)
    }

    #[must_use]
    pub fn download(&self, url: impl IntoUrl, destination: impl AsRef<Path>) -> DownloadBuilder {
        self.inner.download(url, destination)
    }

    #[must_use]
    pub fn upload(&self, url: impl IntoUrl, source: impl AsRef<Path>) -> UploadBuilder {
        self.inner.upload(url, source)
    }

    #[must_use]
    pub fn websocket(&self, url: impl IntoUrl) -> BlockingWebSocketBuilder {
        BlockingWebSocketBuilder::new(self.inner.websocket(url))
    }

    pub fn get_json<T: DeserializeOwned>(&self, url: impl IntoUrl) -> Result<T> {
        runtime::block_on_local(self.inner.get_json(url))
    }

    pub fn post_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        url: impl IntoUrl,
        body: &B,
    ) -> Result<T> {
        runtime::block_on_local(self.inner.post_json(url, body))
    }

    pub fn get_text(&self, url: impl IntoUrl) -> Result<String> {
        runtime::block_on_local(self.inner.get_text(url))
    }

    pub fn get_bytes(&self, url: impl IntoUrl) -> Result<Bytes> {
        runtime::block_on_local(self.inner.get_bytes(url))
    }
}

impl Default for Client {
    fn default() -> Client {
        Client::new()
    }
}
