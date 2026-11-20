use rikka_sync::Mutex;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use http::{HeaderName, HeaderValue, Method, header};
use url::Url;

use crate::Flow;
use crate::auth::Auth;
use crate::cancel::CancelToken;
use crate::client::Client;
use crate::error::Result;
use crate::progress::{Progress, ProgressCallback};
use crate::request::RequestBuilder;
use crate::request_options::RequestOptions;
use crate::response::Response;
use crate::retry::RetryPolicy;
use crate::transfer::{self, Transfer};
use crate::upload::form::Form;
use crate::upload::part::Part;

pub struct UploadBuilder {
    client: Client,
    url: Result<Url>,
    source: PathBuf,
    method: Method,
    field: Option<String>,
    file_name: Option<String>,
    content_type: Option<String>,
    options: RequestOptions,
    progress: Option<ProgressCallback>,
    retry: Option<RetryPolicy>,
}

impl UploadBuilder {
    pub(crate) fn new(client: Client, url: Result<Url>, source: PathBuf) -> UploadBuilder {
        UploadBuilder {
            client,
            url,
            source,
            method: Method::POST,
            field: None,
            file_name: None,
            content_type: None,
            options: RequestOptions::default(),
            progress: None,
            retry: None,
        }
    }

    #[must_use]
    pub fn method(mut self, method: Method) -> Self {
        self.method = method;
        self
    }

    #[must_use]
    pub fn field(mut self, name: impl Into<String>) -> Self {
        self.field = Some(name.into());
        self
    }

    #[must_use]
    pub fn file_name(mut self, name: impl Into<String>) -> Self {
        self.file_name = Some(name.into());
        self
    }

    #[must_use]
    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    #[must_use]
    pub fn header<K, V>(mut self, name: K, value: V) -> Self
    where
        K: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
    {
        self.options.header(name, value);
        self
    }

    #[must_use]
    pub fn auth(mut self, auth: Auth) -> Self {
        self.options.auth(auth);
        self
    }

    #[must_use]
    pub fn bearer_auth(self, token: impl Into<String>) -> Self {
        self.auth(Auth::bearer(token))
    }

    #[must_use]
    pub fn on_progress(mut self, progress: impl FnMut(Progress) -> Flow + Send + 'static) -> Self {
        self.progress = Some(Box::new(progress));
        self
    }

    #[must_use]
    pub fn cancel_token(mut self, token: CancelToken) -> Self {
        self.options.set_cancel(token);
        self
    }

    #[must_use]
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = Some(policy);
        self
    }

    pub async fn send(self) -> Result<Response> {
        self.into_request().await?.send().await
    }

    #[must_use]
    pub fn spawn(mut self) -> Transfer<Response> {
        let shared = Arc::new(Mutex::new(Progress::default()));
        let cancel = self.options.cancel_or_default();

        self.progress = Some(transfer::share(&shared, self.progress.take()));

        Transfer::spawn(shared, cancel, async move { self.send().await })
    }

    async fn into_request(mut self) -> Result<RequestBuilder> {
        self.options.check()?;
        let url = self.url?;

        let mut part = Part::file(&self.source).await?;
        if let Some(content_type) = self.content_type {
            part = part.content_type(content_type);
        }
        if let Some(name) = self.file_name {
            part = part.file_name(name);
        }

        let mut request = self.client.request(self.method, url);

        request = match self.field {
            Some(field) => request.multipart(Form::new().part(field, part)),
            None => {
                let (body, content_type) = part.into_body_and_type();
                request
                    .header(header::CONTENT_TYPE, content_type)
                    .body(body)
            }
        };

        request = self.options.apply(request);
        if let Some(progress) = self.progress {
            request = request.on_upload_progress(progress);
        }

        Ok(request.retry(self.retry.unwrap_or_else(RetryPolicy::never)))
    }
}

impl fmt::Debug for UploadBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UploadBuilder")
            .field("url", &self.url)
            .field("source", &self.source)
            .field("method", &self.method)
            .field("field", &self.field)
            .finish_non_exhaustive()
    }
}
