use std::fmt;
use std::time::Duration;

use http::{HeaderMap, HeaderName, HeaderValue, Version, header};
use serde::Serialize;

use crate::Flow;
use crate::auth::Auth;
use crate::body::Body;
use crate::cancel::CancelToken;
use crate::client::Client;
use crate::error::{Error, Result};
use crate::headers;
use crate::progress::{Progress, ProgressCallback};
use crate::reporter::Reporter;
use crate::response::Response;
use crate::retry::RetryPolicy;
use crate::upload::Form;

pub struct RequestBuilder {
    client: Client,
    inner: Result<reqwest::RequestBuilder>,
    body: Option<Body>,
    upload: Option<ProgressCallback>,
    cancel: Option<CancelToken>,
    retry: Option<RetryPolicy>,
}

impl RequestBuilder {
    pub(crate) fn new(client: Client, inner: Result<reqwest::RequestBuilder>) -> RequestBuilder {
        RequestBuilder {
            client,
            inner,
            body: None,
            upload: None,
            cancel: None,
            retry: None,
        }
    }

    #[must_use]
    pub fn header<K, V>(self, name: K, value: V) -> Self
    where
        K: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
    {
        self.map(|builder| {
            let (name, value) = headers::pair(name, value)?;
            Ok(builder.header(name, value))
        })
    }

    #[must_use]
    pub fn headers(self, headers: HeaderMap) -> Self {
        self.map(|builder| Ok(builder.headers(headers)))
    }

    #[must_use]
    pub fn query<T: Serialize + ?Sized>(self, query: &T) -> Self {
        self.map(|builder| Ok(builder.query(query)))
    }

    #[must_use]
    pub fn auth(self, auth: Auth) -> Self {
        self.map(|builder| {
            let (name, value) = auth.header()?;
            Ok(builder.header(name, value))
        })
    }

    #[must_use]
    pub fn basic_auth(self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.auth(Auth::basic(username, password))
    }

    #[must_use]
    pub fn bearer_auth(self, token: impl Into<String>) -> Self {
        self.auth(Auth::bearer(token))
    }

    #[must_use]
    pub fn body(mut self, body: impl Into<Body>) -> Self {
        self.body = Some(body.into());
        self
    }

    #[must_use]
    pub fn json<T: Serialize + ?Sized>(mut self, value: &T) -> Self {
        match Body::json(value) {
            Ok(body) => {
                self.body = Some(body);
                self.content_type("application/json")
            }
            Err(error) => self.fail(error),
        }
    }

    #[must_use]
    pub fn form<T: Serialize + ?Sized>(mut self, value: &T) -> Self {
        match serde_urlencoded::to_string(value) {
            Ok(encoded) => {
                self.body = Some(Body::text(encoded));
                self.content_type("application/x-www-form-urlencoded")
            }
            Err(error) => self.fail(Error::Encode {
                format: "a form",
                source: Box::new(error),
            }),
        }
    }

    #[must_use]
    pub fn multipart(mut self, form: Form) -> Self {
        let content_type = form.content_type();
        self.body = Some(form.into_body());
        self.content_type(content_type)
    }

    #[must_use]
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.body = Some(Body::text(text));
        self.content_type("text/plain; charset=utf-8")
    }

    #[must_use]
    pub fn timeout(self, timeout: Duration) -> Self {
        self.map(|builder| Ok(builder.timeout(timeout)))
    }

    #[must_use]
    pub fn version(self, version: Version) -> Self {
        self.map(|builder| Ok(builder.version(version)))
    }

    #[must_use]
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = Some(policy);
        self
    }

    #[must_use]
    pub fn cancel_token(mut self, token: CancelToken) -> Self {
        self.cancel = Some(token);
        self
    }

    #[must_use]
    pub fn on_upload_progress(
        mut self,
        progress: impl FnMut(Progress) -> Flow + Send + 'static,
    ) -> Self {
        self.upload = Some(Box::new(progress));
        self
    }

    pub async fn send(self) -> Result<Response> {
        let RequestBuilder {
            client,
            inner,
            body,
            upload,
            cancel,
            retry,
        } = self;

        let mut builder = inner?;

        if let Some(body) = body {
            let body = match upload {
                Some(callback) => {
                    let reporter = Reporter::new(body.length(), 0, Some(callback), cancel.clone());
                    body.metered(reporter)
                }
                None => body,
            };

            let (body, length) = body.into_parts();
            if let Some(length) = length {
                builder = builder.header(header::CONTENT_LENGTH, length);
            }
            builder = builder.body(body);
        }

        let request = builder.build()?;
        client.execute(request, retry, cancel).await
    }

    pub async fn json_response<T: serde::de::DeserializeOwned>(self) -> Result<T> {
        self.send().await?.error_for_status().await?.json().await
    }

    fn content_type(self, value: impl TryInto<HeaderValue>) -> Self {
        self.map(|builder| {
            let value = value
                .try_into()
                .map_err(|_| Error::InvalidHeader("invalid content type".to_owned()))?;

            let mut only = HeaderMap::with_capacity(1);
            only.insert(header::CONTENT_TYPE, value);
            Ok(builder.headers(only))
        })
    }

    fn map(
        mut self,
        with: impl FnOnce(reqwest::RequestBuilder) -> Result<reqwest::RequestBuilder>,
    ) -> Self {
        self.inner = self.inner.and_then(with);
        self
    }

    fn fail(mut self, error: Error) -> Self {
        if self.inner.is_ok() {
            self.inner = Err(error);
        }
        self
    }
}

impl fmt::Debug for RequestBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestBuilder")
            .field("request", &self.inner)
            .field("body", &self.body)
            .finish_non_exhaustive()
    }
}
