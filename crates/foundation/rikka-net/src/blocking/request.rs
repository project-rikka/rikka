use std::time::Duration;

use http::{HeaderMap, HeaderName, HeaderValue, Version};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::Flow;
use crate::auth::Auth;
use crate::blocking::response::BlockingResponse;
use crate::body::Body;
use crate::cancel::CancelToken;
use crate::error::Result;
use crate::progress::Progress;
use crate::request::RequestBuilder;
use crate::retry::RetryPolicy;
use crate::runtime;
use crate::upload::Form;

#[derive(Debug)]
pub struct BlockingRequestBuilder {
    inner: RequestBuilder,
}

impl BlockingRequestBuilder {
    pub(crate) fn new(inner: RequestBuilder) -> BlockingRequestBuilder {
        BlockingRequestBuilder { inner }
    }

    #[must_use]
    pub fn header<K, V>(self, name: K, value: V) -> Self
    where
        K: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
    {
        self.map(|inner| inner.header(name, value))
    }

    #[must_use]
    pub fn headers(self, headers: HeaderMap) -> Self {
        self.map(|inner| inner.headers(headers))
    }

    #[must_use]
    pub fn query<T: Serialize + ?Sized>(self, query: &T) -> Self {
        self.map(|inner| inner.query(query))
    }

    #[must_use]
    pub fn auth(self, auth: Auth) -> Self {
        self.map(|inner| inner.auth(auth))
    }

    #[must_use]
    pub fn basic_auth(self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.map(|inner| inner.basic_auth(username, password))
    }

    #[must_use]
    pub fn bearer_auth(self, token: impl Into<String>) -> Self {
        self.map(|inner| inner.bearer_auth(token))
    }

    #[must_use]
    pub fn body(self, body: impl Into<Body>) -> Self {
        self.map(|inner| inner.body(body))
    }

    #[must_use]
    pub fn json<T: Serialize + ?Sized>(self, value: &T) -> Self {
        self.map(|inner| inner.json(value))
    }

    #[must_use]
    pub fn form<T: Serialize + ?Sized>(self, value: &T) -> Self {
        self.map(|inner| inner.form(value))
    }

    #[must_use]
    pub fn multipart(self, form: Form) -> Self {
        self.map(|inner| inner.multipart(form))
    }

    #[must_use]
    pub fn text(self, text: impl Into<String>) -> Self {
        self.map(|inner| inner.text(text))
    }

    #[must_use]
    pub fn timeout(self, timeout: Duration) -> Self {
        self.map(|inner| inner.timeout(timeout))
    }

    #[must_use]
    pub fn version(self, version: Version) -> Self {
        self.map(|inner| inner.version(version))
    }

    #[must_use]
    pub fn retry(self, policy: RetryPolicy) -> Self {
        self.map(|inner| inner.retry(policy))
    }

    #[must_use]
    pub fn cancel_token(self, token: CancelToken) -> Self {
        self.map(|inner| inner.cancel_token(token))
    }

    #[must_use]
    pub fn on_upload_progress(
        self,
        progress: impl FnMut(Progress) -> Flow + Send + 'static,
    ) -> Self {
        self.map(|inner| inner.on_upload_progress(progress))
    }

    pub fn send(self) -> Result<BlockingResponse> {
        runtime::block_on_local(self.inner.send()).map(BlockingResponse::new)
    }

    pub fn json_response<T: DeserializeOwned>(self) -> Result<T> {
        runtime::block_on_local(self.inner.json_response())
    }

    fn map(mut self, with: impl FnOnce(RequestBuilder) -> RequestBuilder) -> Self {
        self.inner = with(self.inner);
        self
    }
}
